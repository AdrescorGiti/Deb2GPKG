use anyhow::Result;
use std::path::Path;
use crate::manifest::GpkgManifest;
use std::fs;

pub fn generate_fallback_manifest(archive_path: &Path, data_dir: &Path) -> Result<GpkgManifest> {
    let file_stem = archive_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let parts: Vec<&str> = file_stem.split('_').chain(file_stem.split('-')).collect();
    let name = parts.first().unwrap_or(&"unknown").to_string();
    let version = parts.get(1).unwrap_or(&"1.0.0").to_string();
    let architecture = parts.get(2).unwrap_or(&"x86_64").to_string();

    let exec_binary = find_executable(data_dir).unwrap_or_else(|| name.clone());

    Ok(GpkgManifest {
        name: name.clone(),
        version,
        architecture,
        maintainer: "D2G Automated Fallback".to_string(),
        description: format!("Automatically converted from {}", archive_path.display()),
        dependencies: vec![],
        exec_binary,
        installed_files: vec![],
        hooks: Default::default(),
        email: None,
        github: None,
    })
}

fn find_executable(data_dir: &Path) -> Option<String> {
    let bin_dir = data_dir.join("usr/bin");
    if let Some(name) = first_executable_in(&bin_dir) {
        return Some(name);
    }

    let mut found: Option<String> = None;
    let mut stack = vec![data_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "desktop" | "png" | "svg" | "xml" | "mo" | "so" | "dat") {
                    continue;
                }
            }
            let Ok(meta) = fs::metadata(&path) else { continue };
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 == 0 {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                found = Some(name.to_string());
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    found
}

fn first_executable_in(dir: &Path) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    use std::os::unix::fs::PermissionsExt;
    for entry in entries.flatten() {
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_file() {
                if let Ok(meta) = entry.metadata() {
                    if meta.permissions().mode() & 0o111 != 0 {
                        return Some(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}