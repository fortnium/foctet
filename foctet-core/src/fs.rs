use crate::frame::FileMetadata;
use crate::default::DEFAULT_CONFIG_DIR;
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
pub fn get_file_metadata(path: &Path, is_compressed_dir: bool) -> io::Result<FileMetadata> {
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path is not a file",
        ));
    }
    let file_metadata = metadata(path)?;
    let name: String = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();
    let hash = crate::hash::calculate_blake3_file_hash(path)?;
    Ok(FileMetadata {
        name: name,
        size: file_metadata.len(),
        hash: hash,
        is_directory: is_compressed_dir,
    })
}

/// Get config directory path
pub fn get_config_dir_path() -> Option<PathBuf> {
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

/// Get user file path
pub fn get_user_file_path(file_name: &str) -> Option<PathBuf> {
    match get_config_dir_path() {
        Some(mut path) => {
            path.push(file_name);
            Some(path)
        }
        None => None,
    }
}
