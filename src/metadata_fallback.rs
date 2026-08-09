use anyhow::{Context, Result};
use std::path::Path;
use crate::manifest::GpkgManifest;
use std::fs;

pub fn generate_fallback_manifest(archive_path: &Path, data_dir: &Path) -> Result<GpkgManifest> {
    let file_stem = archive_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Infer {name}_{version}_{arch} from filename
    let parts: Vec<&str> = file_stem.split('_').chain(file_stem.split('-')).collect();
    let name = parts.first().unwrap_or(&"unknown").to_string();
    let version = parts.get(1).unwrap_or(&"1.0.0").to_string();
    let architecture = parts.get(2).unwrap_or(&"x86_64").to_string();

    // Scan data/usr/bin/ to find the actual executable
    let exec_binary = find_executable(data_dir).unwrap_or_else(|| name.clone());

    Ok(GpkgManifest {
        name: name.clone(),
        version,
        architecture,
        maintainer: "D2G Automated Fallback".to_string(),
        description: format!("Automatically converted from {}", archive_path.display()),
        dependencies: vec![],
        exec_binary,
        installed_files: vec![], // Populated later
    })
}

fn find_executable(data_dir: &Path) -> Option<String> {
    let bin_dir = data_dir.join("usr/bin");
    if let Ok(entries) = fs::read_dir(bin_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    return Some(entry.file_name().to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}