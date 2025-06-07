use std::fs::{rename, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::{env, fs};

use clap::Parser;
use colorize::{colorize, print_color};
use glob::glob;
use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct HistoryPair(PathBuf, PathBuf);

type HistoryPairs = Vec<HistoryPair>;
type History = Vec<HistoryPairs>;

#[derive(Debug)]
pub struct Trash {
    hist: History,
    hist_path: PathBuf,
    trash_path: PathBuf,
    explain: bool,
}

#[derive(Parser)]
#[command(version, about, long_about=None)]
pub struct Args {
    /// Undo last trash command
    #[arg(long, short)]
    pub undo: bool,

    /// Show full output from command detailing all moves
    #[arg(long, short)]
    pub verbose: bool,

    /// Do not take action, only explain what would occur. Same log level as verbose.
    #[arg(long, short)]
    pub explain: bool,

    /// View history
    #[arg(long, short = 'w')]
    pub view: bool,

    /// Name of file or directory to remove
    #[arg(required_unless_present_any(["undo", "view"]))]
    pub name: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct TrashError(String);
type TrashResult<T> = Result<T, TrashError>;

impl TrashError {
    fn new(err: &str) -> Self {
        Self(err.to_string())
    }
}

impl From<std::io::Error> for TrashError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for TrashError {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for TrashError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

impl std::fmt::Display for TrashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", colorize!(Frb->"trash error:", b->self.0.as_str()))
    }
}

impl Trash {
    #[allow(dead_code)]
    pub(crate) fn new(hist_path: PathBuf, trash_path: PathBuf) -> TrashResult<Self> {
        let file = File::open(&hist_path)?;
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader)?;

        Ok(Self {
            hist_path,
            hist,
            trash_path,
            explain: false,
        })
    }

    pub fn default() -> TrashResult<Self> {
        let (hist_path, trash_path) = resolve_paths()?;
        let file = File::open(&hist_path)?;
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader)?;

        Ok(Self {
            hist_path,
            hist,
            trash_path,
            explain: false,
        })
    }

    pub fn undo(&mut self) -> TrashResult<()> {
        let last = match self.hist.pop() {
            Some(l) => l,
            None => return Err(TrashError::new("No history found!")),
        };

        let mut unresolved: Vec<HistoryPair> = Vec::with_capacity(last.len());

        for l in last {
            let (old, new) = (l.0, l.1);

            info!("{}", colorize!(b->"Moving", Fgb->&new, b->"to", Fgb->&old));

            if self.explain {
                continue;
            }

            if let Err(e) = rename(&new, &old) {
                unresolved.push(HistoryPair(old, new));
                error!("{}", colorize!(Frb->"trash error:", e))
            }
        }

        if !unresolved.is_empty() {
            self.hist.push(unresolved)
        }

        Ok(())
    }

    pub fn remove(&mut self, target: Vec<String>) -> TrashResult<()> {
        let mut hist_item: HistoryPairs = vec![];
        let trash_dir = &self.trash_path;

        // There's no reliable way to tell between normal args and globs, so all are treated as globs
        for t in target {
            for e in glob(&t).expect("Failed to read glob") {
                let old_path = match e {
                    Ok(ent) if ent == self.hist_path => continue,
                    Ok(ent) => ent.canonicalize()?,
                    _ => continue,
                };
                let mut new_path =
                    PathBuf::from_iter([trash_dir.as_os_str(), old_path.file_name().unwrap()]);

                info!(
                    "{}",
                    colorize!(b->"Moving", Fgb->&old_path, b->"to", Fgb->&new_path)
                );

                if self.explain {
                    continue;
                }

                if new_path.exists() {
                    new_path = new_dir_name(new_path);
                    info!(
                        "{}",
                        colorize!(b->"Directory path already exists. Switching to", Fgb->&new_path)
                    );
                }

                // Todo: Better error handling when move doesn't work
                rename(&old_path, &new_path)?;

                let pair = HistoryPair(old_path, new_path);

                hist_item.push(pair);
            }
        }

        self.hist.push(hist_item);

        Ok(())
    }

    pub fn view(&self) {
        for (i, pairs) in self.hist.iter().enumerate() {
            print_color!(NFb->"#", Fbb->i + 1);
            for pair in pairs.iter() {
                print_color!(Fgb->"Moved", b->&pair.0, Fgb->"to", b->&pair.1)
            }
        }
    }

    pub fn write(&self) -> TrashResult<()> {
        let file = File::create(&self.hist_path)?;

        serde_json::to_writer_pretty(file, &self.hist)?;
        Ok(())
    }

    pub fn toggle_explain(&mut self) {
        self.explain = true;
    }
}

pub fn resolve_paths() -> TrashResult<(PathBuf, PathBuf)> {
    let mut hist_path =
        PathBuf::from_iter([dirs::home_dir().unwrap(), PathBuf::from(".config/trash/")]);

    if !hist_path.try_exists().unwrap() {
        fs::create_dir_all(&hist_path)?;
    }

    hist_path.push("trash-history.json");

    if !hist_path.try_exists().unwrap() {
        let mut file = File::create(&hist_path)?;
        file.write_all(b"[]").unwrap();
    }

    let mut trash_dir = env::temp_dir();
    trash_dir.push("trash/");

    // Most likely meaning the computer has restart and /tmp has been cleared
    // New cfg is necessary along with the creation of the directory
    if !trash_dir.try_exists()? {
        fs::create_dir(&trash_dir)?;
        let mut file = File::create(&hist_path)?;
        file.write_all(b"[]")?;
    }

    Ok((hist_path, trash_dir))
}

fn new_dir_name(mut dir: PathBuf) -> PathBuf {
    let mut count = 1;

    loop {
        dir.set_extension(count.to_string());

        if !dir.exists() {
            return dir;
        }

        count += 1;
    }
}
