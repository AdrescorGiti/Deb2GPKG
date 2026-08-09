use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use crate::manifest::GpkgManifest;
use crate::unpacker::PackageUnpacker;

pub struct AppImageUnpacker;

impl PackageUnpacker for AppImageUnpacker {
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest> {
        let data_dir = staging_dir.join("data");
        fs::create_dir_all(&data_dir)?;

        // Даем AppImage права на исполнение (+x)
        if let Ok(metadata) = fs::metadata(archive_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(archive_path, perms);
        }

        // Извлекаем содержимого AppImage через встроенную флаг-команду --appimage-extract
        let status = Command::new(archive_path)
            .arg("--appimage-extract")
            .current_dir(staging_dir)
            .status()
            .context("Не удалось извлечь AppImage")?;

        if !status.success() {
            anyhow::bail!("Ошибка извлечения AppImage");
        }

        let extracted_dir = staging_dir.join("squashfs-root");
        if extracted_dir.exists() {
            // Переносим вытащенную файловую систему в data/
            let _ = fs::rename(&extracted_dir, &data_dir);
        }

        crate::metadata_fallback::generate_fallback_manifest(archive_path, &data_dir)
    }
}