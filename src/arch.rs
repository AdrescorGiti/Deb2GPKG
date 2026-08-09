use anyhow::{Context, Result};
use std::fs::{self, File};
use std::path::Path;
use crate::manifest::GpkgManifest;
use crate::unpacker::PackageUnpacker;

pub struct ArchUnpacker;

impl PackageUnpacker for ArchUnpacker {
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest> {
        let data_dir = staging_dir.join("data");
        fs::create_dir_all(&data_dir).context("Не удалось создать каталог data")?;

        // 1. Открываем файл .pkg.tar.zst и декодируем zstd -> tar
        let file = File::open(archive_path).context("Не удалось открыть пакет Arch")?;
        let zstd_decoder = zstd::stream::Decoder::new(file).context("Ошибка инициализации zstd")?;
        let mut archive = tar::Archive::new(zstd_decoder);

        // 2. Распаковываем файлы в staging/data, пропуская служебные файлы Arch (.PKGINFO и др.)
        for entry_result in archive.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?.to_path_buf();
            
            // Пропускаем метаданные Arch в корне архива
            if path == Path::new(".PKGINFO") 
                || path == Path::new(".BUILDINFO") 
                || path == Path::new(".MTREE") 
                || path.starts_with(".PKGINFO") {
                continue;
            }

            entry.unpack_in(&data_dir)?;
        }

        // 3. Генерируем манифест по распакованным файлам
        crate::metadata_fallback::generate_fallback_manifest(archive_path, &data_dir)
    }
}