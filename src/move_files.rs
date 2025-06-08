use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use colorize::colorize;
use log::{debug, info, warn};

use super::trash::{HistoryPair, HistoryPairs, TrashResult};

pub fn move_targets(
    path: PathBuf,
    base_dir: PathBuf,
    hist_items: &mut HistoryPairs,
    skip_move: bool,
) -> TrashResult<()> {
    debug!("Moving target(s) {:?} - Base Dir: {:?}", &path, &base_dir);

    let mut queue: VecDeque<(PathBuf, PathBuf)> = VecDeque::new();
    queue.push_back((path, base_dir));

    let mut delete_dirs: Vec<PathBuf> = vec![];

    while let Some((item, base)) = queue.pop_front() {
        debug!(
            "Item - {:?}, Base - {:?}, IsDir - {}",
            &item,
            &base,
            item.is_dir()
        );
        let mut new_path = base.join(item.file_name().unwrap());

        if item.is_dir() {
            if new_path.exists() {
                new_item_name(&mut new_path);
                info!(
                    "{}",
                    colorize!(b->"Directory path already exists. Switching to", Fgb->&new_path)
                );
            } else {
                debug!("Creating new dir {:?}", &new_path);
            }

            if !skip_move {
                fs::create_dir_all(&new_path)?;
            }

            let dir_items = fs::read_dir(&item)?
                .filter_map(|ditem| ditem.ok().map(|d| (d.path(), new_path.clone())));
            queue.extend(dir_items);

            delete_dirs.push(item);
        } else if item.is_file() {
            info!(
                "{}",
                colorize!(b->"Moving", Fgb->&item, b->"to", Fgb->&new_path)
            );

            if new_path.exists() {
                new_item_name(&mut new_path);
                info!(
                    "{}",
                    colorize!(b->"File path already exists. Switching to", Fgb->&new_path)
                );
            }

            if skip_move {
                continue;
            }

            rename(&item, &new_path)?;

            let pair = HistoryPair(item, new_path);
            hist_items.push(pair);
        } else {
            warn!("Path {:?} is not a file or a directory. Skipping...", &item);
        }
    }

    for dir in delete_dirs {
        if !dir.exists() {
            continue;
        }
        _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    assert!(from.as_ref().is_file() && !to.as_ref().exists());

    // Open the source file for reading
    let mut source_file = File::open(from.as_ref())?;

    // Create the destination file for writing
    let mut dest_file = File::create(to.as_ref())?;

    // Create a buffer to hold file chunks
    let mut buffer = [0; 8192]; // 8KB buffer

    // Copy data in chunks
    loop {
        let bytes_read = source_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // Reached end of file
        }
        dest_file.write_all(&buffer[..bytes_read])?;
    }

    // Flush to ensure all data is written
    dest_file.flush()?;

    // Close both files (happens automatically when they go out of scope)
    drop(source_file);
    drop(dest_file);

    // Remove the original file
    std::fs::remove_file(from.as_ref())?;

    Ok(())
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
