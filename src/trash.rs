use std::{
    env,
    fs::{self, remove_dir, File},
    io::BufReader,
    path::{Path, PathBuf},
    string::ToString,
};

use clap::Parser;
use colorize::{colorize, print_color};
use glob::{glob, GlobError};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};

use super::move_files::{move_targets, rename};

#[derive(Serialize, Deserialize, Debug)]
pub struct HistoryPair(pub PathBuf, pub PathBuf);

pub type HistoryPairs = Vec<HistoryPair>;
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

#[derive(thiserror::Error, Debug)]
pub enum TrashError {
    #[error("{0}")]
    General(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Serde(#[from] serde_json::error::Error),
    #[error("{0}")]
    Glob(#[from] GlobError),
}

pub type TrashResult<T> = Result<T, TrashError>;

impl TrashError {
    fn new(err: &str) -> Self {
        Self::General(err.to_string())
    }

    pub fn fmt_err(&self) -> String {
        colorize!("{} {}", Frb->"trash error:", b->self.to_string())
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

    pub fn try_new() -> TrashResult<Self> {
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
            None => {
                return Err(TrashError::new("No history found!"));
            }
        };

        let mut unresolved: Vec<HistoryPair> = Vec::with_capacity(last.len());

        for l in last {
            let (old, new) = (l.0, l.1);

            info!(
                "{}",
                colorize!("{} {:?} {} {:?}", b->"Moving", Fgb->&new, b->"to", Fgb->&old)
            );

            if self.explain {
                continue;
            }

            let parent = old.parent().unwrap();

            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }

            if let Err(e) = rename(&new, &old) {
                unresolved.push(HistoryPair(old, new));
                error!("{}", colorize!("{} {}", Frb->"trash error:", e))
            }
        }

        if !unresolved.is_empty() {
            self.hist.push(unresolved)
        }

        clean_trash_dir(&self.trash_path);

        Ok(())
    }

    pub fn remove(&mut self, target: Vec<String>) -> TrashResult<()> {
        let mut hist_item: HistoryPairs = vec![];
        let trash_dir = &self.trash_path;

        // There's no reliable way to tell between normal args and globs, so all are treated as globs
        for t in target {
            let glob_paths = match glob(&t) {
                Ok(paths) => paths,
                Err(e) => {
                    error!("Glob error - {}", e);
                    continue;
                }
            };

            for e in glob_paths {
                let old_path = match e {
                    Ok(ent) if ent == self.hist_path || ent.starts_with(trash_dir) => continue,
                    Ok(ent) => ent.canonicalize()?,
                    Err(e) => {
                        error!("Glob error - {}", e);
                        continue;
                    }
                };

                move_targets(old_path, trash_dir.clone(), &mut hist_item, self.explain)?;
            }
        }

        self.hist.push(hist_item);

        Ok(())
    }

    pub fn view(&self) {
        for (i, pairs) in self.hist.iter().enumerate() {
            print_color!("{} {}", NFb->"#", Fbb->i + 1);
            for pair in pairs.iter() {
                print_color!("{} {:?} {} {:?}", Fgb->"Moved", b->&pair.0, Fgb->"to", b->&pair.1)
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

fn resolve_paths() -> TrashResult<(PathBuf, PathBuf)> {
    let trash_dir = env::temp_dir().join("trash");
    let hist_path = dirs::data_local_dir()
        .ok_or(TrashError::General("Could not get local data dir".into()))?
        .join("trash-rs")
        .join("trash-history.json");

    debug!(
        "Trash Directory - {:?}, History Path - {:?}",
        &trash_dir, &hist_path
    );

    if !hist_path.try_exists()? {
        fs::create_dir_all(hist_path.parent().unwrap())?;
        fs::write(&hist_path, b"[]")?;
    }

    if !trash_dir.try_exists()? {
        fs::create_dir_all(&trash_dir)?;
    }

    Ok((hist_path, trash_dir))
}

fn clean_trash_dir(dir: &Path) {
    let read_dir = fs::read_dir(dir);

    if read_dir.is_err() {
        return;
    }

    read_dir
        .unwrap()
        .filter_map(|ent| ent.ok())
        .for_each(|ent| {
            let ent_path = ent.path();
            println!("{:?}", ent_path);

            if ent_path.is_dir() && remove_dir(&ent_path).is_err() {
                clean_trash_dir(&ent_path);
            }
        });
}
