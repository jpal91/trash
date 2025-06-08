mod move_files;
mod trash;

use std::{io::Write, process::ExitCode};

use clap::Parser;
use colorize::colorize;
use env_logger::Builder;
use log::{error, info, LevelFilter};
use stacker;

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

    let mut trash = match Trash::default() {
        Ok(t) => t,
        Err(e) => {
            error!("{}", e);
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
            colorize!(Fyb->"Explain mode - No actions will be taken")
        );
        trash.toggle_explain();
    }

    let res = stacker::maybe_grow(32 * 1024, 50 * 1024 * 1024, || {
        if args.undo {
            if let Err(e) = trash.undo() {
                error!("{}", e);
                return Err(e);
            }
        } else if let Err(e) = trash.remove(args.name.unwrap()) {
            error!("{}", e);
            // return ExitCode::FAILURE;
            return Err(e);
        }

        Ok(())
    });

    if res.is_err() {
        return ExitCode::FAILURE;
    }

    if !args.explain {
        if let Err(e) = trash.write() {
            error!("{}", e);
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir;
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
            .map(|f| f.unwrap().file_name().to_str().unwrap().to_string())
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
            PathBuf::from("dev/trash/dev-folder/"),
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

    #[test]
    fn test_non_empty_directory_doesnt_fail() {
        let (tmp_dir, hist_path) = trash_dir();
        let mut trash_dir = tmp_dir.path().to_owned();
        let mut test_dir = tmp_dir.path().to_owned();

        trash_dir.push("trash_dir");
        test_dir.push("test_dir");

        let non_empty_dir = test_dir.clone().join("non-empty");
        create_dir(&non_empty_dir).unwrap();

        let mut non_empty_dir2 = trash_dir.clone().join("non-empty");
        create_dir(&non_empty_dir2).unwrap();

        let mut f1 = File::create(non_empty_dir.join("test1.txt")).unwrap();
        f1.write_all(b"stuff").unwrap();

        let mut f2 = File::create(non_empty_dir2.join("test1.txt")).unwrap();
        f2.write_all(b"stuff").unwrap();

        env::set_current_dir(&test_dir).unwrap();

        let mut trash = Trash::new(hist_path.clone(), trash_dir.clone()).unwrap();

        trash.remove(vec!["non-empty".to_string()]).unwrap();

        assert!(!non_empty_dir.exists());

        assert!(non_empty_dir2.exists());

        non_empty_dir2.pop();

        assert!(non_empty_dir2.join("non-empty.1").exists());
    }
}
