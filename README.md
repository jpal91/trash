# trash

A friendlier implementation of the `rm` command. 

## Purpose

`rm` can be a little scary sometimes and I wanted an implementation that I didn't have to stare at for a full minute to make sure I was absolutely doing the correct thing before pressing enter. 

With `trash`, instead of the item(s) being permanently deleted, they will be moved to a directory created by `trash`, which will be stored in the user's `tmp` directory. If an item was `trashed` by mistake, it can be moved back (and `trash` even can help with that!). Once the user's computer restarts, the item(s) will be deleted per normal OS process, but it still gives the user time/ability to correct any mistakes. 

## Features
- Ability to send one or more files to the `trash`
    - Just like with `rm` each argument is going to be acted on
    - Supports UNIX globbing
- History
    - `undo` the last action
    - `view` the history
- Ultimately the files will be deleted, just like with `rm` if the user takes no action
- Speed and safety of Rust
- Explain Mode - Can tell you what will happen instead of performing the action
- Pretty color output

## Usage
```bash
# Moves my_doc.txt to the trash
trash my_doc.txt

# Moves all contents of my_dir to the trash, leaving it empty. -v (verbose) logs each move as it happens.
trash -v my_dir/*

# Undo last move, moving the contents of my_dir back to it's original place(s)
trash -u

# Don't move anything, but log on the console what would happen in this command
trash -e my_dir/*

# View the history of all your prior trash commands
trash -w

# View help and exit
trash -h
```

## Installation

Requires `rustc` and `Cargo` installed on your computer.

```bash
git clone https://github.com/jpal91/trash.git
cd trash
cargo install --path .
```

## Hacking - OS

Currently this is built for Linux, but should be able to be re-tooled somewhat easily for Mac and Windows.

Mainly the `resolve_paths` function would just have to be implemented to fit the OS. Anything that is written in by hand would have to be altered to fit the desired path.

```rust
pub fn resolve_paths() -> TrashResult<(PathBuf, PathBuf)> {
    let mut hist_path = PathBuf::from_iter([
        dirs::home_dir().unwrap(), // This should be fine as it should work on Windows/Mac
        PathBuf::from(".config/trash/") // This would have to be changed
    ]);
    ...
    let mut trash_dir = env::temp_dir(); // This should also work
    trash_dir.push("trash/"); // may need to change
    ...
}
```

