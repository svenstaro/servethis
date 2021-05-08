use actix_web::http::ContentEncoding;
use libflate::gzip::Encoder;
use serde::Deserialize;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::path::PathBuf;
use strum_macros::{Display, EnumIter, EnumString};
use tar::Builder;
use zip::{write, ZipWriter};

use crate::errors::ContextualError;

/// Available archive methods
#[derive(Deserialize, Clone, Copy, EnumIter, EnumString, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ArchiveMethod {
    /// Gzipped tarball
    TarGz,

    /// Regular tarball
    Tar,

    /// Regular zip
    Zip,
}

impl ArchiveMethod {
    pub fn extension(self) -> String {
        match self {
            ArchiveMethod::TarGz => "tar.gz",
            ArchiveMethod::Tar => "tar",
            ArchiveMethod::Zip => "zip",
        }
        .to_string()
    }

    pub fn content_type(self) -> String {
        match self {
            ArchiveMethod::TarGz => "application/gzip",
            ArchiveMethod::Tar => "application/tar",
            ArchiveMethod::Zip => "application/zip",
        }
        .to_string()
    }

    pub fn content_encoding(self) -> ContentEncoding {
        match self {
            ArchiveMethod::TarGz => ContentEncoding::Gzip,
            ArchiveMethod::Tar => ContentEncoding::Identity,
            ArchiveMethod::Zip => ContentEncoding::Identity,
        }
    }

    pub fn is_enabled(self, tar_enabled: bool, tar_gz_enabled: bool, zip_enabled: bool) -> bool {
        match self {
            ArchiveMethod::TarGz => tar_gz_enabled,
            ArchiveMethod::Tar => tar_enabled,
            ArchiveMethod::Zip => zip_enabled,
        }
    }

    /// Make an archive out of the given directory, and write the output to the given writer.
    ///
    /// Recursively includes all files and subdirectories.
    ///
    /// If `skip_symlinks` is `true`, symlinks fill not be followed and will just be ignored.
    pub fn create_archive<T, W>(
        self,
        dir: T,
        skip_symlinks: bool,
        out: W,
    ) -> Result<(), ContextualError>
    where
        T: AsRef<Path>,
        W: std::io::Write,
    {
        let dir = dir.as_ref();
        match self {
            ArchiveMethod::TarGz => tar_gz(dir, skip_symlinks, out),
            ArchiveMethod::Tar => tar_dir(dir, skip_symlinks, out),
            ArchiveMethod::Zip => zip_dir(dir, skip_symlinks, out),
        }
    }
}

/// Write a gzipped tarball of `dir` in `out`.
fn tar_gz<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let mut out = Encoder::new(out).map_err(|e| ContextualError::IoError("GZIP".to_string(), e))?;

    tar_dir(dir, skip_symlinks, &mut out)?;

    out.finish()
        .into_result()
        .map_err(|e| ContextualError::IoError("GZIP finish".to_string(), e))?;

    Ok(())
}

/// Write a tarball of `dir` in `out`.
///
/// The target directory will be saved as a top-level directory in the archive.
///
/// For example, consider this directory structure:
///
/// ```ignore
/// a
/// └── b
///     └── c
///         ├── e
///         ├── f
///         └── g
/// ```
///
/// Making a tarball out of `"a/b/c"` will result in this archive content:
///
/// ```ignore
/// c
/// ├── e
/// ├── f
/// └── g
/// ```
fn tar_dir<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let inner_folder = dir.file_name().ok_or_else(|| {
        ContextualError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    let directory = inner_folder.to_str().ok_or_else(|| {
        ContextualError::InvalidPathError(
            "Directory name contains invalid UTF-8 characters".to_string(),
        )
    })?;

    tar(dir, directory.to_string(), skip_symlinks, out)
        .map_err(|e| ContextualError::ArchiveCreationError("tarball".to_string(), Box::new(e)))
}

/// Writes a tarball of `dir` in `out`.
///
/// The content of `src_dir` will be saved in the archive as a folder named `inner_folder`.
fn tar<W>(
    src_dir: &Path,
    inner_folder: String,
    skip_symlinks: bool,
    out: W,
) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let mut tar_builder = Builder::new(out);

    tar_builder.follow_symlinks(!skip_symlinks);

    // Recursively adds the content of src_dir into the archive stream
    tar_builder
        .append_dir_all(inner_folder, src_dir)
        .map_err(|e| {
            ContextualError::IoError(
                format!(
                    "Failed to append the content of {} to the TAR archive",
                    src_dir.to_str().unwrap_or("file")
                ),
                e,
            )
        })?;

    // Finish the archive
    tar_builder.into_inner().map_err(|e| {
        ContextualError::IoError("Failed to finish writing the TAR archive".to_string(), e)
    })?;

    Ok(())
}

/// Write a zip of `dir` in `out`.
///
/// The target directory will be saved as a top-level directory in the archive.
///
/// For example, consider this directory structure:
///
/// ```ignore
/// a
/// └── b
///     └── c
///         ├── e
///         ├── f
///         └── g
/// ```
///
/// Making a zip out of `"a/b/c"` will result in this archive content:
///
/// ```ignore
/// c
/// ├── e
/// ├── f
/// └── g
/// ```
fn create_zip_from_directory<W>(
    out: W,
    directory: &Path,
    skip_symlinks: bool,
) -> Result<(), ContextualError>
where
    W: std::io::Write + std::io::Seek,
{
    let options = write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut paths_queue: Vec<PathBuf> = vec![directory.to_path_buf()];
    let zip_root_folder_name = directory.file_name().ok_or_else(|| {
        ContextualError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    let mut zip_writer = ZipWriter::new(out);
    let mut buffer = Vec::new();
    while !paths_queue.is_empty() {
        let next = paths_queue.pop().ok_or_else(|| {
            ContextualError::ArchiveCreationDetailError("Could not get path from queue".to_string())
        })?;
        let current_dir = next.as_path();
        let directory_entry_iterator = std::fs::read_dir(current_dir)
            .map_err(|e| ContextualError::IoError("Could not read directory".to_string(), e))?;
        let zip_directory = Path::new(zip_root_folder_name).join(
            current_dir.strip_prefix(directory).map_err(|_| {
                ContextualError::ArchiveCreationDetailError(
                    "Could not append base directory".to_string(),
                )
            })?,
        );

        for entry in directory_entry_iterator {
            let entry_path = entry
                .ok()
                .ok_or_else(|| {
                    ContextualError::InvalidPathError(
                        "Directory name terminates in \"..\"".to_string(),
                    )
                })?
                .path();
            let entry_metadata = std::fs::metadata(entry_path.clone()).map_err(|e| {
                ContextualError::IoError("Could not get file metadata".to_string(), e)
            })?;

            if entry_metadata.file_type().is_symlink() && skip_symlinks {
                continue;
            }
            let current_entry_name = entry_path.file_name().ok_or_else(|| {
                ContextualError::InvalidPathError("Invalid file or direcotory name".to_string())
            })?;
            if entry_metadata.is_file() {
                let mut f = File::open(&entry_path)
                    .map_err(|e| ContextualError::IoError("Could not open file".to_string(), e))?;
                f.read_to_end(&mut buffer).map_err(|e| {
                    ContextualError::IoError("Could not read from file".to_string(), e)
                })?;
                let relative_path = zip_directory.join(current_entry_name).into_os_string();
                zip_writer
                    .start_file(relative_path.to_string_lossy(), options)
                    .map_err(|_| {
                        ContextualError::ArchiveCreationDetailError(
                            "Could not add file path to ZIP".to_string(),
                        )
                    })?;
                zip_writer.write(buffer.as_ref()).map_err(|_| {
                    ContextualError::ArchiveCreationDetailError(
                        "Could not write file to ZIP".to_string(),
                    )
                })?;
                buffer.clear();
            } else if entry_metadata.is_dir() {
                let relative_path = zip_directory.join(current_entry_name).into_os_string();
                zip_writer
                    .add_directory(relative_path.to_string_lossy(), options)
                    .map_err(|_| {
                        ContextualError::ArchiveCreationDetailError(
                            "Could not add directory path to ZIP".to_string(),
                        )
                    })?;
                paths_queue.push(entry_path.clone());
            }
        }
    }

    zip_writer.finish().map_err(|_| {
        ContextualError::ArchiveCreationDetailError(
            "Could not finish writing ZIP archive".to_string(),
        )
    })?;
    Ok(())
}

/// Writes a zip of `dir` in `out`.
///
/// The content of `src_dir` will be saved in the archive as the  folder named .
fn zip_data<W>(src_dir: &Path, skip_symlinks: bool, mut out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let mut data = Vec::new();
    let memory_file = Cursor::new(&mut data);
    create_zip_from_directory(memory_file, &src_dir.to_path_buf(), skip_symlinks).map_err(|e| {
        ContextualError::ArchiveCreationError(
            "Failed to create the ZIP archive".to_string(),
            Box::new(e),
        )
    })?;

    out.write_all(data.as_mut_slice())
        .map_err(|e| ContextualError::IoError("Failed to write the ZIP archive".to_string(), e))?;

    Ok(())
}

fn zip_dir<W>(dir: &Path, skip_symlinks: bool, out: W) -> Result<(), ContextualError>
where
    W: std::io::Write,
{
    let inner_folder = dir.file_name().ok_or_else(|| {
        ContextualError::InvalidPathError("Directory name terminates in \"..\"".to_string())
    })?;

    inner_folder.to_str().ok_or_else(|| {
        ContextualError::InvalidPathError(
            "Directory name contains invalid UTF-8 characters".to_string(),
        )
    })?;

    zip_data(dir, skip_symlinks, out)
        .map_err(|e| ContextualError::ArchiveCreationError("zip".to_string(), Box::new(e)))
}
