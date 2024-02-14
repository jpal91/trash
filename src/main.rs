
use std::{env, fs};
use std::fs::{File, rename};
use std::path::PathBuf;
use std::io::{BufReader, Write};
use std::process::ExitCode;

use clap::Parser;
use serde::{Serialize, Deserialize};
use glob::glob;
use log::{info, error, LevelFilter};
use env_logger::Builder;
use colorize::{colorize, print_color};

#[derive(Serialize, Deserialize, Debug)]
struct HistoryPair(PathBuf, PathBuf);

type HistoryPairs = Vec<HistoryPair>;
type History = Vec<HistoryPairs>;

#[derive(Debug)]
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

    /// View history
    #[arg(long, short='w')]
    view: bool,

    /// Name of file or directory to remove
    #[arg(required_unless_present_any(["undo", "view"]))]
    name: Option<Vec<String>>
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
    pub fn new(hist_path: PathBuf, trash_path: PathBuf) -> TrashResult<Self> {
        let file = File::open(&hist_path)?;
        let reader = BufReader::new(file);

        let hist: History = serde_json::from_reader(reader)?;

        Ok(Self {
            hist_path,
            hist,
            trash_path,
            explain: false
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
            explain: false
        })
    }

    pub fn undo(&mut self) -> TrashResult<()> {
        let last = match self.hist.pop() {
            Some(l) => l,
            None => return Err(TrashError::new("No history found!"))
        };

        let mut unresolved: Vec<HistoryPair> = Vec::with_capacity(last.len());

        for l in last {
            let (old, new) = (l.0, l.1);

            info!("{}", colorize!(b->"Moving", Fgb->&new, b->"to", Fgb->&old));

            if self.explain {
                continue
            }
            
            if let Err(e) = rename(&new, &old) {
                unresolved.push(HistoryPair(old, new));
                error!("{}", colorize!(Frb->"trash error:", e))
            }
            
        };

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
                    _ => continue
                };
                let new_path = PathBuf::from_iter([trash_dir.as_os_str(), old_path.file_name().unwrap()]);
                

                info!("{}", colorize!(b->"Moving", Fgb->&old_path, b->"to", Fgb->&new_path));

                if self.explain {
                    continue
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
    let mut hist_path = PathBuf::from_iter([
        dirs::home_dir().unwrap(),
        PathBuf::from(".config/trash/")
    ]);

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


fn main() -> ExitCode {
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

    let mut trash = match Trash::default() {
        Ok(t) => t,
        Err(e) => {
            error!("{}", e);
            return ExitCode::FAILURE
        }
    };

    if args.view {
        trash.view();
        return ExitCode::SUCCESS
    }

    if args.explain {
        info!("{}", colorize!(Fyb->"Explain mode - No actions will be taken"));
        trash.toggle_explain();
    }

    if args.undo {
        if let Err(e) = trash.undo() {
            error!("{}", e);
            return ExitCode::FAILURE
        }
    } else {
        if let Err(e) = trash.remove(args.name.unwrap()) {
            error!("{}", e);
            return ExitCode::FAILURE
        }
    }

    if !args.explain {
        if let Err(e) = trash.write() {
            error!("{}", e);
            return ExitCode::FAILURE
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir;

    fn trash_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp_dir = tempfile::tempdir().unwrap();
        let mut tmp_path = tmp_dir.path().to_owned();

        tmp_path.push("trash-history.json");

        let hist_path = tmp_path.clone();

        let mut cfg = File::create(&tmp_path).unwrap();
        cfg.write_all(b"[]").unwrap();
        tmp_path.pop();

        tmp_path.push("test_dir");
        create_dir(&tmp_path).unwrap();

        for i in 0..3 {
            tmp_path.push(format!("test{}.txt", i));
            File::create(&tmp_path).unwrap();
            tmp_path.pop();
        }

        tmp_path.pop();
        tmp_path.push("trash_dir");
        create_dir(&tmp_path).unwrap();
    

        (tmp_dir, hist_path)
    }

    #[test]
    fn test_trash() {
    
        let (tmp_dir, hist_path) = trash_dir();
        let mut trash_dir = tmp_dir.path().to_owned();
        let mut test_dir = tmp_dir.path().to_owned();

        trash_dir.push("trash_dir");
        test_dir.push("test_dir");

        env::set_current_dir(&test_dir).unwrap();

        let files: Vec<String> = test_dir
            .read_dir()
            .unwrap()
            .map(|f|{
                f.unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        let mut trash = Trash::new(hist_path.to_owned(), trash_dir.to_owned()).unwrap();

        trash.remove(files.clone()).unwrap();

        for file in files {
            test_dir.push(file);
            assert!(!test_dir.exists());
            test_dir.pop();
        }
    }

    #[test]
    fn test_glob() {
        let (tmp_dir, hist_path) = trash_dir();
        let mut trash_dir = tmp_dir.path().to_owned();
        let mut test_dir = tmp_dir.path().to_owned();

        trash_dir.push("trash_dir");

        env::set_current_dir(&test_dir).unwrap();

        let mut trash = Trash::new(hist_path.to_owned(), trash_dir.to_owned()).unwrap();

        trash.remove(vec![String::from("test_dir/*")]).unwrap();

        test_dir.push("test_dir");

        for i in 0..3 {
            test_dir.push(format!("test{}.txt", i));
            assert!(!test_dir.exists());
            test_dir.pop();
        }
    }

    #[test]
    fn test_trash_explain() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().to_owned();

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();
        trash.toggle_explain();

        let mut target_fld = PathBuf::from_iter([
            dirs::home_dir().unwrap(),
            PathBuf::from("dev/trash/dev-folder/")
        ]);

        target_fld.push("two.txt");

        let mv_file = vec![target_fld.to_string_lossy().to_string()];

        trash.remove(mv_file).unwrap();

        assert!(target_fld.exists());
    }

    #[test]
    fn test_undo() {
        let (tmp_dir, hist_path) = trash_dir();
        let mut trash_dir = tmp_dir.path().to_owned();
        let mut test_dir = tmp_dir.path().to_owned();

        trash_dir.push("trash_dir");
        test_dir.push("test_dir");

        env::set_current_dir(&test_dir).unwrap();

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();

        test_dir.push("test1.txt");

        trash.remove(vec!["test1.txt".to_string()]).unwrap();

        assert!(!test_dir.exists());

        trash.undo().unwrap();

        assert!(test_dir.exists())
    }
}