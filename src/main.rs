mod move_files;
mod trash;

use std::{io::Write, process::ExitCode};

use clap::Parser;
use colorize::colorize;
use env_logger::Builder;
use log::{error, info, LevelFilter};

use trash::{Args, Trash};

fn main() -> ExitCode {
    let args = Args::parse();

    let mut logger = Builder::new();

    logger.format(|buf, record| writeln!(buf, "{}", record.args()));

    if args.verbose {
        logger.filter_level(LevelFilter::Debug);
    } else if args.explain {
        logger.filter_level(LevelFilter::Info);
    }

    logger.init();

    let mut trash = match Trash::try_new() {
        Ok(t) => t,
        Err(e) => {
            error!("{}", e.fmt_err());
            return ExitCode::FAILURE;
        }
    };

    if args.view {
        trash.view();
        return ExitCode::SUCCESS;
    }

    if args.explain {
        info!(
            "{}",
            colorize!("{}", Fyb->"Explain mode - No actions will be taken")
        );
        trash.toggle_explain();
    }

    if args.undo {
        if let Err(e) = trash.undo() {
            error!("{}", e.fmt_err());
            return ExitCode::FAILURE;
        }
    } else if let Err(e) = trash.remove(args.name.unwrap()) {
        error!("{}", e.fmt_err());
        return ExitCode::FAILURE;
    }

    if !args.explain {
        if let Err(e) = trash.write() {
            error!("{}", e.fmt_err());
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use std::fs;
    use std::{env, fs::File, path::PathBuf};

    fn trash_dir() -> (tempfile::TempDir, PathBuf) {
        let tmp_dir = tempfile::tempdir().unwrap();
        let mut tmp_path = tmp_dir.path().to_owned();

        tmp_path.push("trash-history.json");

        let hist_path = tmp_path.clone();

        let mut cfg = File::create(&tmp_path).unwrap();
        cfg.write_all(b"[]").unwrap();
        tmp_path.pop();

        tmp_path.push("test_dir");
        fs::create_dir(&tmp_path).unwrap();
        env::set_current_dir(&tmp_path).unwrap();

        for i in 0..3 {
            tmp_path.push(format!("test{}.txt", i));
            File::create(&tmp_path).unwrap();
            tmp_path.pop();
        }

        tmp_path.pop();
        tmp_path.push("trash_dir");
        fs::create_dir(&tmp_path).unwrap();

        (tmp_dir, hist_path)
    }

    fn fill_dir(tmp_dir: &TempDir) -> PathBuf {
        let id = Uuid::new_v4();
        let path = tmp_dir.path().join("test_dir").join(id.to_string());
        fs::create_dir(&path).unwrap();

        for i in 0..3 {
            fs::write(path.join(format!("test-{}.txt", i)), "").unwrap();
        }

        path
    }

    #[test]
    fn test_trash() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().join("trash_dir");
        let mut test_dir = tmp_dir.path().join("test_dir");
        let new_dir = fill_dir(&tmp_dir);

        let files: Vec<String> = new_dir
            .read_dir()
            .unwrap()
            .map(|f| {
                format!(
                    "{}/{}",
                    new_dir.to_string_lossy(),
                    f.unwrap().file_name().to_string_lossy()
                )
            })
            .collect();

        let mut trash = Trash::new(hist_path.to_owned(), trash_dir.to_owned()).unwrap();

        trash.remove(files.clone()).unwrap();

        test_dir.push(new_dir);

        for file in files {
            test_dir.push(file);
            assert!(!test_dir.exists());
            test_dir.pop();
        }
    }

    #[test]
    fn test_glob() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().join("trash_dir");
        let mut test_dir = tmp_dir.path().join("test_dir");
        let new_dir = fill_dir(&tmp_dir);

        let mut trash = Trash::new(hist_path.to_owned(), trash_dir.to_owned()).unwrap();

        let dirname = new_dir.file_name().unwrap().to_string_lossy();
        trash.remove(vec![format!("{}/*", dirname)]).unwrap();

        for i in 0..3 {
            test_dir.push(format!("test-{}.txt", i));
            assert!(!test_dir.exists());
            test_dir.pop();
        }
    }

    #[test]
    fn test_trash_explain() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().join("trash_dir");
        let new_dir = fill_dir(&tmp_dir);

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();
        trash.toggle_explain();

        let target_fld = new_dir.join(&new_dir).join("test-1.txt");

        let mv_file = vec![target_fld.to_string_lossy().to_string()];

        trash.remove(mv_file).unwrap();

        assert!(target_fld.exists());
    }

    #[test]
    fn test_undo() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().join("trash_dir");
        let mut test_dir = tmp_dir.path().join("test_dir");
        let new_dir = fill_dir(&tmp_dir);

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();

        test_dir.push(&new_dir);
        test_dir.push("test-1.txt");

        trash
            .remove(vec![format!("{}/test-1.txt", new_dir.to_string_lossy())])
            .unwrap();

        assert!(!test_dir.exists());

        trash.undo().unwrap();

        assert!(test_dir.exists())
    }

    #[test]
    fn test_non_empty_directory_doesnt_fail() {
        let (tmp_dir, hist_path) = trash_dir();
        let trash_dir = tmp_dir.path().join("trash_dir");
        let new_dir = fill_dir(&tmp_dir);

        let non_empty_dir = new_dir.join("non-empty");
        fs::create_dir(&non_empty_dir).unwrap();

        let mut non_empty_dir2 = trash_dir.join("non-empty");
        fs::create_dir(&non_empty_dir2).unwrap();

        fs::write(non_empty_dir.join("test-non-empty.txt"), "").unwrap();
        fs::write(non_empty_dir2.join("test-non-empty.txt"), "").unwrap();

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();

        trash
            .remove(vec![format!("{}/non-empty", new_dir.to_string_lossy())])
            .unwrap();

        assert!(!non_empty_dir.exists());

        assert!(non_empty_dir2.exists());

        non_empty_dir2.pop();

        assert!(non_empty_dir2.join("non-empty.1").exists());
    }
}
