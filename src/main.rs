use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context as _, Result};
use clap::{Parser, ValueEnum};
use itertools::Itertools as _;
use petgraph::{dot::Dot, visit::EdgeRef};

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

fn build_dbg(
    tokens: &[&str],
    separator: &str,
    context: usize,
) -> petgraph::graph::DiGraph<String, u16> {
    let mut nodes = HashMap::new();

    tokens.windows(context).array_windows::<2>().fold(
        petgraph::graph::DiGraph::new(),
        |mut g, [cur, next]| {
            let cur_node = *nodes
                .entry(cur)
                .or_insert_with(|| g.add_node(cur.join(separator)));

            let next_node = *nodes
                .entry(next)
                .or_insert_with(|| g.add_node(next.join(separator)));

            let edge_weight = g
                .edges_connecting(cur_node, next_node)
                .next()
                .map_or(0, |e| *e.weight());

            g.update_edge(cur_node, next_node, edge_weight + 1);

            g
        },
    )
}

fn parse_weights(context: usize, mode: Mode, input: &str) -> petgraph::graph::DiGraph<String, u16> {
    match mode {
        Mode::Words => {
            let tokens = input.split_whitespace().collect_vec();

            build_dbg(&tokens, " ", context)
        }
        Mode::Letters => {
            let tokens = input.split("").filter(|s| !s.is_empty()).collect_vec();

            build_dbg(&tokens, "", context)
        }
    }
}

#[allow(dead_code)]
fn dbg_to_dot(dbg: &petgraph::graph::DiGraph<String, u16>) -> String {
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
    dbg: &petgraph::graph::DiGraph<String, u16>,
    start: petgraph::graph::NodeIndex,
    mode: Mode,
    length: usize,
) {
    print!("{}", dbg.node_weight(start).unwrap());
    let mut next = start;
    for _ in 0..length {
        let outgoing_edges = dbg
            .edges_directed(next, petgraph::Direction::Outgoing)
            .collect_vec();

        if outgoing_edges.is_empty() {
            break;
        }

        let total_weight = outgoing_edges.iter().map(|e| *e.weight()).sum::<u16>() as usize;
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

            match mode {
                Mode::Words => &format!(
                    " {}",
                    raw_weight.split_whitespace().next_back().unwrap_or("")
                ),
                Mode::Letters => raw_weight
                    .split("")
                    .filter(|s| !s.is_empty())
                    .last()
                    .unwrap_or(""),
            }
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

    let dbg = parse_weights(context, mode, &std::fs::read_to_string(file)?);

    let start = fastrand::choice(dbg.node_indices()).context("no nodes")?;
    // let start = petgraph::graph::NodeIndex::new(0);
    print_markov(&dbg, start, mode, length);
    // let dot = dbg_to_dot(&dbg);
    // println!("{}", dot);

    Ok(())
}
