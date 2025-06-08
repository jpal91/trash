#![allow(unused)]
use std::{
    collections::VecDeque,
    env,
    fs::{self, File},
    io::{BufReader, Write},
    path::PathBuf,
    string::ToString,
};

use clap::Parser;
use colorize::{colorize, print_color};
// use fs_extra::{
//     dir::{move_dir, CopyOptions},
//     file::{move_file, CopyOptions as FileCopyOpts},
// };
use fs_more::{
    directory::{move_directory, DirectoryMoveOptions},
    file::{move_file, FileMoveOptions},
};
use glob::{glob, GlobError};
use log::{debug, error, info, warn};
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

// #[derive(Debug)]
// pub struct TrashError(String);

#[derive(thiserror::Error, Debug)]
pub enum TrashError {
    #[error("{}", self.fmt_err())]
    General(String),
    #[error("{}", self.fmt_err())]
    Io(#[from] std::io::Error),
    #[error("{}", self.fmt_err())]
    Serde(#[from] serde_json::error::Error),
    #[error("{}", self.fmt_err())]
    Glob(#[from] GlobError),
    #[error("{}", self.fmt_err())]
    FSExtra(#[from] fs_extra::error::Error),
    #[error("{}", self.fmt_err())]
    FSMore(#[from] fs_more::error::FileError),
    #[error("{}", self.fmt_err())]
    FSMoreDir(#[from] fs_more::error::MoveDirectoryError),
}

type TrashResult<T> = Result<T, TrashError>;

impl TrashError {
    fn new(err: &str) -> Self {
        Self::General(err.to_string())
    }

    fn fmt_err(&self) -> String {
        colorize!(Frb->"trash error:", b->self.to_string())
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
        let (hist_path, trash_path) = resolve_paths_new()?;
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

            if let Err(e) = fs::rename(&new, &old) {
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
                let mut new_path = trash_dir.join(old_path.file_name().unwrap());
                // let mut new_path =
                //     PathBuf::from_iter([trash_dir.as_os_str(), old_path.file_name().unwrap()]);

                info!(
                    "{}",
                    colorize!(b->"Moving", Fgb->&old_path, b->"to", Fgb->&new_path)
                );

                if self.explain {
                    continue;
                }

                if new_path.exists() {
                    // new_path = new_dir_name(new_path);
                    new_item_name(&mut new_path);
                    info!(
                        "{}",
                        colorize!(b->"Directory path already exists. Switching to", Fgb->&new_path)
                    );
                }

                // // Todo: Better error handling when move doesn't work
                fs::rename(&old_path, &new_path)?;

                // if old_path.is_file() {
                //     move_file(&old_path, &new_path, FileMoveOptions::default())?;
                // } else if old_path.is_dir() {
                //     move_directory(&old_path, &new_path, DirectoryMoveOptions::default())?;
                // } else {
                //     warn!(
                //         "Path {:?} is not a file or a directory. Skipping...",
                //         &old_path
                //     );
                //     continue;
                // }

                let pair = HistoryPair(old_path, new_path);
                hist_item.push(pair);
                // move_targets(old_path, trash_dir.clone(), &mut hist_item, self.explain)?;
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

fn resolve_paths_new() -> TrashResult<(PathBuf, PathBuf)> {
    let hist_path = env::temp_dir().join("trash-rs-history.json");
    let trash_dir = dirs::cache_dir()
        .ok_or(TrashError::General("Could not get local data dir".into()))?
        .join("trash-rs-dump");

    if !hist_path.try_exists()? {
        _ = fs::remove_dir_all(&trash_dir);
        fs::write(&hist_path, b"[]")?;
    }

    if !trash_dir.try_exists()? {
        fs::create_dir_all(&trash_dir)?;
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

fn new_item_name(item: &mut PathBuf) {
    let mut count = 1;

    loop {
        item.set_extension(count.to_string());

        if !item.exists() {
            break;
        }

        count += 1;
    }
}

// fn move_target(target_path: PathBuf, base_dir: PathBuf) -> TrashResult<()> {
//     if target_path.is_file() {
//         let mut new_path =
//             PathBuf::from_iter([base_dir.as_os_str(), target_path.file_name().unwrap()]);

//         if new_path.exists() {
//             new_path = new_dir_name(new_path);
//         }

//         move_file(&target_path, &new_path, FileMoveOptions::default())?;
//     } else if target_path.is_dir() {
//         move_dir(target_path, base_dir)?;
//     } else {
//         warn!(
//             "Path {:?} is not a file or a directory. Skipping...",
//             &old_path
//         );
//     }

//     Ok(())
// }

fn move_targets(
    path: PathBuf,
    base_dir: PathBuf,
    hist_items: &mut HistoryPairs,
    skip_move: bool,
) -> TrashResult<()> {
    debug!("Moving target(s) {:?} - Base Dir: {:?}", &path, &base_dir);

    let mut queue: VecDeque<(PathBuf, PathBuf)> = VecDeque::new();
    queue.push_back((path, base_dir));

    while let Some((item, base)) = queue.pop_front() {
        debug!("Item - {:?}, Base - {:?}", &item, &base);

        if item.is_dir() {
            let mut new_path = PathBuf::from_iter([base.as_os_str(), item.file_name().unwrap()]);

            if !base.exists() {
                debug!("Creating new temp dir {:?}", &base);

                if !skip_move {
                    fs::create_dir_all(&base)?;
                }
            }

            if new_path.exists() {
                new_path = new_dir_name(new_path);
                info!(
                    "{}",
                    colorize!(b->"Directory path already exists. Switching to", Fgb->&new_path)
                );
            }

            let dir_items = fs::read_dir(&item)?
                .filter_map(|ditem| ditem.ok().map(|d| (d.path(), new_path.clone())));
            queue.extend(dir_items);
        } else if item.is_file() {
            let mut new_path = PathBuf::from_iter([base.as_os_str(), item.file_name().unwrap()]);

            info!(
                "{}",
                colorize!(b->"Moving", Fgb->&item, b->"to", Fgb->&new_path)
            );

            if new_path.exists() {
                new_path = new_dir_name(new_path);
                info!(
                    "{}",
                    colorize!(b->"Directory path already exists. Switching to", Fgb->&new_path)
                );
            }

            if skip_move {
                continue;
            }

            // move_file(&item, &new_path, FileMoveOptions::default())?;

            fs::copy(&item, &new_path)?;

            fs::remove_file(&item)?;

            let pair = HistoryPair(item, new_path);
            hist_items.push(pair);
        } else {
            warn!("Path {:?} is not a file or a directory. Skipping...", &item);
        }
    }
    Ok(())
}
