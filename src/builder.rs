use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn build_gpkg(staging: &Path, output: &Path) -> Result<()> {
    let out_file = File::create(output).context("Failed to create output .gpkg file")?;
    
    // Менеджер gvalli использует формат GZIP (.tar.gz)
    let encoder = GzEncoder::new(out_file, Compression::default());
    let mut tar_builder = tar::Builder::new(encoder);

    // 1. Упаковываем манифест
    let manifest_path = staging.join("manifest.json");
    tar_builder.append_path_with_name(&manifest_path, "manifest.json")?;

    // 2. Упаковываем хуки (если есть)
    let hooks_dir = staging.join("hooks");
    if hooks_dir.exists() {
        tar_builder.append_dir_all("hooks", &hooks_dir)?;
    }

    // 3. Упаковываем данные с обязательным префиксом "files/"
    let data_dir = staging.join("data");
    if data_dir.exists() {
        tar_builder.append_dir_all("files", &data_dir)?;
    }

    // 4. Закрываем tar, завершаем сжатие GZIP и сбрасываем буфер на накопитель
    let gz_encoder = tar_builder.into_inner().context("Failed to finish tar archive")?;
    let mut file = gz_encoder.finish().context("Failed to finish GZIP stream")?;
    file.flush().context("Failed to flush file to disk")?;

    Ok(())
}