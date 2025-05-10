use crate::default::DEFAULT_CONFIG_DIR;
use crate::hash::Blake3Hasher;
use crate::metadata::FileMetadata;
//use crate::time::UnixTimestamp;
use anyhow::anyhow;
use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{metadata, File};
use std::io;
use std::path::{Path, PathBuf};
use tar::Builder;

/// Compresses a directory (recursively) into a .tar.gz archive
pub fn compress_directory(dir_path: &Path, output_file: &Path) -> io::Result<()> {
    // Open the output .tar.gz file
    let tar_gz = File::create(output_file)?;
    let encoder = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(encoder);

    // Add all files in the directory to the .tar.gz archive (recursively)
    tar.append_dir_all(".", dir_path)?;

    // Finish the .tar.gz archive
    tar.finish()?;

    Ok(())
}

/// Get file or directory metadata
pub fn get_file_metadata(path: &Path) -> Result<FileMetadata> {
    if !path.is_file() {
        return Err(anyhow!("Path is not a file"));
    }
    let file_metadata = metadata(path)?;
    let name: String = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();
    let hasher: Blake3Hasher = Blake3Hasher::new();
    let hash = hasher.calculate_file_hash(path)?;

    // Timestamps
    // let created: UnixTimestamp = UnixTimestamp::from_system_time(file_metadata.created()?);
    // let modified: UnixTimestamp = UnixTimestamp::from_system_time(file_metadata.modified()?);
    // let accessed: UnixTimestamp = UnixTimestamp::from_system_time(file_metadata.accessed()?);

    Ok(FileMetadata {
        name: name,
        size: file_metadata.len() as usize,
        hash: hash,
    })
}

/// Get user config directory path
pub fn get_user_data_dir_path() -> Option<PathBuf> {
    match home::home_dir() {
        Some(mut path) => {
            path.push(DEFAULT_CONFIG_DIR);
            if !path.exists() {
                match std::fs::create_dir_all(&path) {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to create config directory: {}", e);
                        return None;
                    }
                }
            }
            Some(path)
        }
        None => None,
    }
}

/// Get user data file/directory path
/// The 'relative_path' should be a relative path to the user data directory.
/// Returns the full path if the user data directory exists, otherwise None.
/// e.g. get_user_data_path("logs/foctet.log") -> Some("/home/user/.foctet/logs/foctet.log")
pub fn get_user_data_path(relative_path: &PathBuf) -> Option<PathBuf> {
    match get_user_data_dir_path() {
        Some(mut path) => {
            path.push(relative_path);
            Some(path)
        }
        None => None,
    }
}
