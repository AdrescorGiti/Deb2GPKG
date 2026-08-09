use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;

pub fn build_gpkg(staging: &Path, output: &Path) -> Result<()> {
    let out_file = File::create(output).context("Failed to create output .gpkg file")?;
    
    // ZSTD compression with default level (3)
    let encoder = zstd::stream::Encoder::new(out_file, 3)
        .context("Failed to initialize ZSTD encoder")?
        .auto_finish();
        
    let mut tar_builder = tar::Builder::new(encoder);

    // Append the manifest
    let manifest_path = staging.join("manifest.json");
    tar_builder.append_path_with_name(&manifest_path, "manifest.json")?;

    // Append hooks if they exist
    let hooks_dir = staging.join("hooks");
    if hooks_dir.exists() {
        tar_builder.append_dir_all("hooks", &hooks_dir)?;
    }

    // Append the payload data directly to the root of the archive
    let data_dir = staging.join("data");
    if data_dir.exists() {
        tar_builder.append_dir_all("data", &data_dir)?;
    }

    tar_builder.into_inner().context("Failed to finish tar archive")?;

    Ok(())
}