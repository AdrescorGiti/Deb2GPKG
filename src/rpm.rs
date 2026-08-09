use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::manifest::GpkgManifest;
use crate::unpacker::PackageUnpacker;

pub struct RpmUnpacker;

impl PackageUnpacker for RpmUnpacker {
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest> {
        let data_dir = staging_dir.join("data");
        fs::create_dir_all(&data_dir).context("Не удалось создать папку data")?;

        // 1. Пробуем распаковать RPM через bsdtar (есть в CachyOS/Arch по умолчанию)
        let bsdtar_result = Command::new("bsdtar")
            .arg("-x")
            .arg("-C")
            .arg(&data_dir)
            .arg("-f")
            .arg(archive_path)
            .status();

        let mut success = matches!(bsdtar_result, Ok(status) if status.success());

        // 2. Фолбэк: если bsdtar не справился, используем связку rpm2cpio + cpio
        if !success {
            let cpio_cmd = format!(
                "rpm2cpio '{}' | (cd '{}' && cpio -idmv)",
                archive_path.to_string_lossy(),
                data_dir.to_string_lossy()
            );

            let cpio_status = Command::new("sh")
                .arg("-c")
                .arg(&cpio_cmd)
                .status()
                .context("Не удалось выполнить распаковку через rpm2cpio")?;

            success = cpio_status.success();
        }

        if !success {
            anyhow::bail!("Не удалось распаковать RPM пакет. Убедитесь, что установлены bsdtar или rpm2cpio.");
        }

        // 3. Генерируем манифест на основе сканирования распакованных фалов в data/
        crate::metadata_fallback::generate_fallback_manifest(archive_path, &data_dir)
    }
}