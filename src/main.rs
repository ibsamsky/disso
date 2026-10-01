use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context as _, Result};
use clap::{Parser, ValueEnum};
use itertools::Itertools as _;
use petgraph::{dot::Dot, visit::EdgeRef};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Words,
    Letters,
}

#[derive(Parser, Debug)]
struct Args {
    /// Number of tokens to generate before stopping.
    #[arg(short, long)]
    length: usize,
    /// Whether to use words or letters as tokens.
    #[arg(short, long, value_enum, default_value_t = Mode::Words)]
    mode: Mode,
    /// How many tokens to use as context for generating the next token.
    #[arg(short, long, default_value_t = 3)]
    context: usize,
    /// Input file.
    #[arg(short, long)]
    file: PathBuf,
}

fn build_dbg<'v, 's>(
    tokens: &'s [&'v str],
    context: usize,
) -> petgraph::graph::DiGraph<&'s [&'v str], u16> {
    let (_, g) = tokens
        .par_windows(context)
        .zip(tokens.par_windows(context).skip(1))
        .fold_with(
            (HashMap::new(), petgraph::graph::DiGraph::new()),
            |(mut nodes, mut g), (cur, next)| {
                let cur_node = *nodes.entry(cur).or_insert_with(|| g.add_node(cur));
                let next_node = *nodes.entry(next).or_insert_with(|| g.add_node(next));

                let edge_weight = g
                    .edges_connecting(cur_node, next_node)
                    .next()
                    .map_or(0, |e| *e.weight());

                g.update_edge(cur_node, next_node, edge_weight + 1);

                (nodes, g)
            },
        )
        .reduce(
            || (HashMap::new(), petgraph::graph::DiGraph::new()),
            |mut left, right| {
                for (rsource, rtarget, rweight) in right
                    .1
                    .edge_references()
                    .map(|e| (e.source(), e.target(), *e.weight()))
                {
                    let rsource_weight = right.1.node_weight(rsource).unwrap();
                    let rtarget_weight = right.1.node_weight(rtarget).unwrap();

                    let lsource = *left
                        .0
                        .entry(rsource_weight)
                        .or_insert_with(|| left.1.add_node(rsource_weight));

                    let ltarget = *left
                        .0
                        .entry(rtarget_weight)
                        .or_insert_with(|| left.1.add_node(rtarget_weight));

                    let edge_weight = left
                        .1
                        .edges_connecting(lsource, ltarget)
                        .next()
                        .map_or(0, |e| *e.weight());

                    left.1.update_edge(lsource, ltarget, edge_weight + rweight);
                }

                left
            },
        );

    g
}

#[allow(dead_code)]
fn dbg_to_dot(dbg: &petgraph::graph::DiGraph<&[&str], u16>) -> String {
    let max_weight = dbg.edge_weights().max().copied().unwrap_or(1) as f64;
    let max_penwidth = 5.0;

    format!(
        "{:?}",
        Dot::with_attr_getters(
            dbg,
            &[],
            &|_, e| {
                format!(
                    "penwidth={} len={}",
                    (*e.weight() as f64 * max_penwidth) / max_weight,
                    max_weight + 1. - *e.weight() as f64
                )
            },
            &|_, (_, _)| { "".to_string() }
        )
    )
}

#[allow(dead_code)]
fn print_markov(
    dbg: &petgraph::graph::DiGraph<&[&str], u16>,
    start: petgraph::graph::NodeIndex,
    mode: Mode,
    length: usize,
) {
    let separator = match mode {
        Mode::Words => " ",
        Mode::Letters => "",
    };

    print!("{}", dbg.node_weight(start).unwrap().join(separator));
    let mut next = start;
    for _ in 0..length {
        let outgoing_edges = dbg
            .edges_directed(next, petgraph::Direction::Outgoing)
            .collect_vec();

        if outgoing_edges.is_empty() {
            break;
        }

        let total_weight = outgoing_edges
            .iter()
            .map(|e| *e.weight() as u32)
            .sum::<u32>() as usize;
        let idx = fastrand::usize(0..total_weight);

        let mut cumulative_weight = 0;
        for edge in outgoing_edges {
            cumulative_weight += *edge.weight() as usize;
            if cumulative_weight >= idx {
                next = edge.target();
                break;
            }
        }

        let transition = {
            let raw_weight = dbg.node_weight(next).unwrap();

            let last = raw_weight.last().unwrap();
            format!("{separator}{last}")
        };
        print!("{}", transition);
    }
    println!();
}

fn main() -> Result<()> {
    let args = Args::try_parse()?;

    let Args {
        length,
        mode,
        context,
        file,
    } = args;

    let time = std::time::Instant::now();

    let input = std::fs::read_to_string(&file).context("failed to read input file")?;

    let tokens = match mode {
        Mode::Words => input.split_whitespace().collect_vec(),
        Mode::Letters => input.split("").filter(|s| !s.is_empty()).collect_vec(),
    };
    let dbg = build_dbg(&tokens, context);

    eprintln!("Parsed {} nodes in {:?}", dbg.node_count(), time.elapsed());

    let start = fastrand::choice(dbg.node_indices()).context("no nodes")?;
    // let start = petgraph::graph::NodeIndex::new(0);
    print_markov(&dbg, start, mode, length);
    // let dot = dbg_to_dot(&dbg);
    // println!("{}", dot);

    Ok(())
}
