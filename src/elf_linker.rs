use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Scans the extracted data directory for ELF binaries, resolves their dynamic
/// dependencies via `ldd`, copies missing `.so` libraries into a private bundle
/// directory (`/opt/gpkg_libs/<pkg_name>`), and patches the ELF RPATH/RUNPATH.
/// This ensures "Swiss watch" precision out-of-the-box on G OS.
pub fn bundle_and_patch_elfs(data_dir: &Path, pkg_name: &str) -> Result<()> {
    let elf_files = find_elf_files(data_dir)?;
    if elf_files.is_empty() {
        return Ok(());
    }

    // Isolate dependencies in a package-specific directory to prevent conflicts
    // in the pristine G OS environment.
    let lib_bundle_rel = format!("opt/gpkg_libs/{}", pkg_name);
    let lib_bundle_abs = data_dir.join(&lib_bundle_rel);
    fs::create_dir_all(&lib_bundle_abs).context("Failed to create isolated library directory")?;

    for elf_path in elf_files {
        // 1. Resolve dependencies using host's ldd tool
        let deps = resolve_dependencies(&elf_path)?;
        
        for dep in deps {
            let dep_name = dep.file_name().unwrap_or_default();
            let dest_lib = lib_bundle_abs.join(dep_name);
            
            // Only copy if it doesn't already exist in our bundle and exists on host
            if !dest_lib.exists() && dep.exists() {
                let _ = fs::copy(&dep, &dest_lib);
            }
        }

        // 2. Calculate relative path from the ELF binary to the lib bundle
        // so we can set $ORIGIN-based RPATH.
        if let Ok(elf_rel) = elf_path.strip_prefix(data_dir) {
            // Count depth to root of data_dir
            let depth = elf_rel.components().count().saturating_sub(1);
            let up_dirs = "../".repeat(depth);
            let rpath = format!("$ORIGIN/{}{}", up_dirs, lib_bundle_rel);

            // 3. Patch the binary using patchelf
            patch_rpath(&elf_path, &rpath)?;
        }
    }

    Ok(())
}

fn find_elf_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut elf_files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_elf_binary(&path) {
                elf_files.push(path);
            }
        }
    }
    Ok(elf_files)
}

/// Simple heuristic to check for ELF magic bytes without dragging in heavy crates.
fn is_elf_binary(path: &Path) -> bool {
    if let Ok(data) = fs::read(path) {
        if data.len() >= 4 {
            return data[0..4] == [0x7f, 0x45, 0x4c, 0x46]; // \x7F E L F
        }
    }
    false
}

fn resolve_dependencies(elf_path: &Path) -> Result<Vec<PathBuf>> {
    let mut deps = Vec::new();
    let output = Command::new("ldd")
        .arg(elf_path)
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            // Parse lines like: "libz.so.1 => /lib/x86_64-linux-gnu/libz.so.1 (0x...)"
            if let Some(arrow_idx) = line.find("=>") {
                let path_part = line[arrow_idx + 2..].trim();
                if let Some(space_idx) = path_part.find(' ') {
                    let so_path = &path_part[..space_idx];
                    if so_path != "not found" {
                        deps.push(PathBuf::from(so_path));
                    }
                }
            }
        }
    }
    Ok(deps)
}

fn patch_rpath(elf_path: &Path, rpath: &str) -> Result<()> {
    // Requires 'patchelf' to be installed on the host building the packages.
    let status = Command::new("patchelf")
        .arg("--set-rpath")
        .arg(rpath)
        .arg(elf_path)
        .status();

    if let Ok(s) = status {
        if !s.success() {
            eprintln!("Warning: Failed to patch RPATH for {}", elf_path.display());
        }
    }
    Ok(())
}