
use std::{env, fs};
use std::fs::{File, rename};
use std::path::PathBuf;
use std::io::{BufReader, Write};

use clap::Parser;
use serde::{Serialize, Deserialize};
use glob::glob;
use log::{info, error, LevelFilter};
use env_logger::Builder;
use colorize::colorize;

#[derive(Serialize, Deserialize)]
struct HistoryPair(PathBuf, PathBuf);

type HistoryPairs = Vec<HistoryPair>;
type History = Vec<HistoryPairs>;

pub struct Trash {
    hist: History,
    hist_path: PathBuf,
    trash_path: PathBuf,
    explain: bool
}


#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    /// Undo last trash command
    #[arg(long, short)]
    undo: bool,

    /// Show full output from command detailing all moves
    #[arg(long, short)]
    verbose: bool,

    /// Do not take action, only explain what would occur. Same log level as verbose.
    #[arg(long, short)]
    explain: bool,

    /// Name of file or directory to remove
    #[arg(required_unless_present = "undo")]
    name: Option<Vec<String>>
}

impl Trash {
    pub fn new(hist_path: PathBuf, trash_path: PathBuf) -> Self {
        let file = File::open(&hist_path).unwrap();
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader).unwrap();

        Self {
            hist_path,
            hist,
            trash_path,
            explain: false
        }
    }

    pub fn undo(&mut self) {
        let last = match self.hist.pop() {
            Some(l) => l,
            None => return error!("{}", colorize!(Frb->"trash error:", b->"No history found!"))
        };

        let mut unresolved: Vec<HistoryPair> = Vec::with_capacity(last.len());

        for l in last {
            let (old, new) = (l.0, l.1);

            if !self.explain && move_file(&new, &old).is_err() {
                unresolved.push(HistoryPair(old, new));
                continue
            }

            info!("{}", colorize!(b->"Moving", Fgb->new, b->"to", Fgb->old))
        };

        if !unresolved.is_empty() {
            self.hist.push(unresolved)
        }
    }

    pub fn remove(&mut self, target: Vec<String>) {
        let mut hist_item: HistoryPairs = vec![];
        let trash_dir = &self.trash_path;

        // There's no reliable way to tell between normal args and globs, so all are treated as globs
        for t in target {
            for e in glob(&t).expect("Failed to read glob") {
                let old_path = match e {
                    Ok(ent) if ent == self.hist_path => continue,
                    Ok(ent) => ent.canonicalize().unwrap(),
                    _ => continue
                };
                let new_path = PathBuf::from_iter([trash_dir.as_os_str(), old_path.file_name().unwrap()]);
                
                if !self.explain && move_file(&old_path, &new_path).is_err() {
                    continue
                }

                info!("{}", colorize!(b->"Moving", Fgb->&old_path, b->"to", Fgb->&new_path));

                let pair = HistoryPair(old_path, new_path);

                hist_item.push(pair);
            }
        }

        self.hist.push(hist_item)
    }

    pub fn write(&self) {
        let file = File::create(&self.hist_path).unwrap();

        serde_json::to_writer_pretty(file, &self.hist).unwrap();
    }

    pub fn toggle_explain(&mut self) {
        self.explain = true;
    }
}

impl Default for Trash {
    fn default() -> Self {
        let (hist_path, trash_path) = resolve_paths();
        let file = File::open(&hist_path).unwrap();
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader).unwrap();

        Self {
            hist_path,
            hist,
            trash_path,
            explain: false
        }
    }
}

pub fn resolve_paths() -> (PathBuf, PathBuf) {
    let mut hist_path = PathBuf::from_iter([
        dirs::home_dir().unwrap(),
        PathBuf::from(".config/trash/")
    ]);

    if !hist_path.try_exists().unwrap() {
        fs::create_dir_all(&hist_path).unwrap();
    }

    hist_path.push("trash-history.json");

    if !hist_path.try_exists().unwrap() {
        let mut file = File::create(&hist_path).unwrap();
        file.write_all(b"[]").unwrap();
    }

    let mut trash_dir = env::temp_dir();
    trash_dir.push("trash/");

    // Most likely meaning the computer has restart and /tmp has been cleared
    // New cfg is necessary along with the creation of the directory
    if !trash_dir.try_exists().unwrap() {
        fs::create_dir(&trash_dir).unwrap();
        let mut file = File::create(&hist_path).unwrap();
        file.write_all(b"[]").unwrap();
    }

    (hist_path, trash_dir)
}

#[allow(clippy::result_unit_err)]
pub fn move_file(src: &PathBuf, dst: &PathBuf) -> Result<(), ()> {
    if let Err(e) =  rename(src, dst) {
        error!("{}", colorize!(Frb->"trash error:", "Unable to move", b->src, e));
        Err(())
    } else {
        Ok(())
    }
}

fn main() {
    let args = Args::parse();

    let mut logger = Builder::new();

    logger.format(|buf, record| {
        writeln!(
            buf,
            "{}",
            record.args()
        )
    });

    if args.verbose || args.explain {
        logger.filter_level(LevelFilter::Debug);
    };

    logger.init();

    let mut trash = Trash::default();

    if args.explain {
        info!("{}", colorize!(Fyb->"Explain mode - No actions will be taken"));
        trash.toggle_explain();
    }

    if args.undo {
        trash.undo();
    } else {
        trash.remove(args.name.unwrap())
    }

    if !args.explain {
        trash.write();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_paths_debug() -> (PathBuf, PathBuf) {
        let trash_dir = PathBuf::from_iter([
            dirs::home_dir().unwrap(),
            PathBuf::from("dev/trash/dev-trash/")
        ]);

        let mut hist_path = trash_dir.clone();
        hist_path.push("trash-history.json");

        (hist_path, trash_dir)
    }

    #[test]
    fn test_trash_explain() {
        let (hist_path, trash_dir) = resolve_paths_debug();

        Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}",
                    record.args()
                )
            })
            .filter_level(LevelFilter::Debug)
            .init();

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone());
        trash.toggle_explain();

        let mut target_fld = PathBuf::from_iter([
            dirs::home_dir().unwrap(),
            PathBuf::from("dev/trash/dev-folder/")
        ]);

        target_fld.push("two.txt");

        let mv_file = vec![target_fld.to_string_lossy().to_string()];

        trash.remove(mv_file);

        assert!(target_fld.exists());
    }
}