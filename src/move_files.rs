use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

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
