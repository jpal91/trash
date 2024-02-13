#![allow(unused)]
use std::{env, fs};
use std::os::fd::{AsFd, AsRawFd};
use std::{fs::File, str::FromStr, fs::rename};
use std::path::{PathBuf, Path};
use std::io::{BufReader, Write};
use std::collections::HashMap;

use clap::Parser;
use serde::{Serialize, Deserialize};
use glob::glob;
use dirs;
use log::{info, debug, error, LevelFilter, trace};
use env_logger::{Builder};

#[derive(Serialize, Deserialize, Debug)]
struct HistoryPair(PathBuf, PathBuf);

type HistoryPairs = Vec<HistoryPair>;
type History = Vec<HistoryPairs>;

struct Trash {
    hist: History,
    cfg_path: PathBuf,
    trash_path: PathBuf
}


#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// Undo last trash command
    #[arg(long, short)]
    undo: bool,

    #[arg(long, short)]
    verbose: bool,

    /// Name of file or directory to remove
    #[arg(required_unless_present = "undo")]
    name: Option<Vec<String>>
}

impl Trash {
    fn new(cfg_path: PathBuf, trash_path: PathBuf) -> Self {
        let file = File::open(&cfg_path).unwrap();
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader).unwrap();

        Self {
            cfg_path,
            hist,
            trash_path
        }
    }

    fn undo(&mut self) {
        let last = match self.hist.pop() {
            Some(l) => l,
            None => return error!("trash error: No history found!")
        };

        let mut unresolved: Vec<HistoryPair> = Vec::with_capacity(last.len());

        for l in last {
            let (old, new) = (l.0, l.1);

            if let Err(e) =  rename(&new, &old) {
                error!("trash error: Unable to move {:?} - {}", &new, e);
                unresolved.push(HistoryPair(old, new));
                continue
            };

            info!("Moving {:?} to {:?}", new, old);
        };

        if !unresolved.is_empty() {
            self.hist.push(unresolved)
        }
    }

    fn remove(&mut self, target: Vec<String>) {
        let mut hist_item: HistoryPairs = vec![];
        let trash_dir = &self.trash_path;

        for t in target {
            for e in glob(&t).expect("Failed to read glob") {
                let old_path = match e {
                    Ok(ent) if ent == self.cfg_path => continue,
                    Ok(ent) => ent.canonicalize().unwrap(),
                    _ => continue
                };
                let new_path = PathBuf::from_iter([trash_dir.as_os_str(), old_path.file_name().unwrap()]);
                
                if let Err(e) =  rename(&old_path, &new_path) {
                    error!("trash error: Unable to move {:?} - {}", &old_path, e);
                    continue
                };

                info!("Moving {:?} to {:?}", &old_path, &new_path);

                let pair = HistoryPair(old_path, new_path);

                hist_item.push(pair);
            }
        }

        self.hist.push(hist_item)
    }

    fn write(&self) {
        let file = File::create(&self.cfg_path).unwrap();

        serde_json::to_writer_pretty(file, &self.hist).unwrap();
    }
}

fn resolve_paths() -> (PathBuf, PathBuf) {
    let mut cfg_path = PathBuf::from_iter([
        dirs::home_dir().unwrap(),
        PathBuf::from(".config/trash/")
    ]);

    if !cfg_path.try_exists().unwrap() {
        fs::create_dir_all(&cfg_path).unwrap();
    }

    cfg_path.push("trash-history.json");

    if !cfg_path.try_exists().unwrap() {
        File::create(&cfg_path).unwrap();
    }

    let mut trash_dir = env::temp_dir();
    trash_dir.push("trash/");

    if !trash_dir.try_exists().unwrap() {
        fs::create_dir(&trash_dir).unwrap();
    }

    (cfg_path, trash_dir)
}

fn main() {
    let args = Args::parse();
    let (cfg_path, trash_path) = resolve_paths();

    let mut logger = Builder::new();

    logger.format(|buf, record| {
        writeln!(
            buf,
            "{}",
            record.args()
        )
    });

    if args.verbose {
        logger.filter_level(LevelFilter::Debug);
    };

    logger.init();

    let mut trash = Trash::new(cfg_path, trash_path);

    if args.undo {
        trash.undo();
    } else {
        trash.remove(args.name.unwrap())
    }

    trash.write();
}