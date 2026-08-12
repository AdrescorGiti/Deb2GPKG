use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

pub fn build_gpkg(staging: &Path, output: &Path) -> Result<()> {
    let out_file = File::create(output).context("Failed to create output .gpkg file")?;

    let encoder = GzEncoder::new(out_file, Compression::default());
    let mut tar_builder = tar::Builder::new(encoder);

    // 1. GPKGM manifest — всегда первый в архиве
    let manifest_path = staging.join("GPKGM");
    append_file(&mut tar_builder, &manifest_path, "GPKGM")?;

    // 2. Hooks
    let hooks_dir = staging.join("hooks");
    if hooks_dir.exists() {
        append_tree(&mut tar_builder, &hooks_dir, "hooks")?;
    }

    // 3. Payload под префиксом `files/`
    let data_dir = staging.join("data");
    if data_dir.exists() {
        append_tree(&mut tar_builder, &data_dir, "files")?;
    }

    let gz_encoder = tar_builder.into_inner().context("Failed to finish tar archive")?;
    let mut file = gz_encoder.finish().context("Failed to finish GZIP stream")?;
    file.flush().context("Failed to flush file to disk")?;
    file.sync_all().context("Failed to fsync .gpkg to disk")?;

    Ok(())
}

fn append_file(builder: &mut tar::Builder<GzEncoder<File>>, src: &Path, archive_name: &str) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    let metadata = std::fs::metadata(src)
        .with_context(|| format!("stat {}", src.display()))?;
    header.set_size(metadata.len());
    header.set_mode(metadata.permissions().mode());
    header.set_mtime(metadata.mtime() as u64);
    header.set_cksum();
    let mut f = File::open(src)?;
    builder.append_data(&mut header, archive_name, &mut f)?;
    Ok(())
}

fn append_tree(
    builder: &mut tar::Builder<GzEncoder<File>>,
    root: &Path,
    archive_prefix: &str,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .contents_first(false)
    {
        let entry = entry?;
        let path = entry.path();
        if path == root {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(Path::new(""));
        let archive_path = PathBuf::from(archive_prefix).join(rel);

        let metadata = entry.metadata()?;
        let mut header = tar::Header::new_gnu();
        header.set_mtime(metadata.mtime() as u64);

        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            let target = std::fs::read_link(path)
                .with_context(|| format!("readlink {}", path.display()))?;
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_cksum();
            builder.append_link(&mut header, archive_path, &target)?;
        } else if file_type.is_dir() {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(metadata.permissions().mode());
            header.set_cksum();
            builder.append_data(&mut header, &archive_path, std::io::empty())?;
        } else if file_type.is_file() {
            header.set_size(metadata.len());
            header.set_mode(metadata.permissions().mode());
            header.set_cksum();
            let mut f = File::open(path)?;
            builder.append_data(&mut header, &archive_path, &mut f)?;
        }
    }
    Ok(())
}