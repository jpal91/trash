#![allow(unused)]

use std::collections::HashMap;

use clap::Parser;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct HistoryPair(String, String);

type HistoryPairs = Vec<HistoryPair>;
type History = Vec<HistoryPairs>;


#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// Undo last trash command
    #[arg(long, short)]
    undo: bool,

    /// Name of file or directory to remove
    #[arg(required_unless_present = "undo")]
    name: Option<String>
}

fn main() {
    let args = Args::parse();
}