#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod builder;
mod manifest;
mod deb;
mod unpacker;
mod sanitizer;
mod metadata_fallback;
mod elf_linker;

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

fn collect_installed_files(data_dir: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    fn recurse(dir: &Path, base: &Path, list: &mut Vec<String>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    recurse(&path, base, list)?;
                } else {
                    if let Ok(rel) = path.strip_prefix(base) {
                        list.push(format!("/{}", rel.display()));
                    }
                }
            }
        }
        Ok(())
    }
    recurse(data_dir, data_dir, &mut files)?;
    Ok(files)
}

fn ensure_exec_integration(data_dir: &Path, manifest: &mut manifest::GpkgManifest) -> Result<()> {
    let bin_dir = data_dir.join("usr/bin");
    fs::create_dir_all(&bin_dir)?;

    let mut exec_name = manifest.exec_binary.clone();
    if exec_name.is_empty() {
        exec_name = manifest.name.clone();
    }

    let direct = bin_dir.join(&exec_name);
    
    // UNIVERSAL FALLBACK: Если главный бинарник не найден, находим любой валидный ELF.
    if !direct.exists() {
        let candidates = find_executable_candidates(data_dir, &exec_name);
        if let Some(real_path) = candidates.first() {
            let rel = real_path.strip_prefix(data_dir).unwrap_or(real_path);
            let abs_target = Path::new("/").join(rel);
            
            let fallback_name = real_path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let link = bin_dir.join(&fallback_name);
            
            let _ = fs::remove_file(&link);
            let _ = symlink(&abs_target, &link);
            
            manifest.exec_binary = fallback_name;
        }
    }

    // Generic fix for Electron Apps sandboxing
    enforce_electron_sandbox_permissions(data_dir)?;

    Ok(())
}

fn enforce_electron_sandbox_permissions(data_dir: &Path) -> Result<()> {
    let mut stack = vec![data_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("chrome-sandbox") {
                let Ok(metadata) = fs::metadata(&path) else { continue };
                let mut perms = metadata.permissions();
                perms.set_mode(0o4755); // SUID root bit
                let _ = fs::set_permissions(&path, perms);
            }
        }
    }
    Ok(())
}

fn find_executable_candidates(root: &Path, preferred: &str) -> Vec<PathBuf> {
    let mut matches: Vec<PathBuf> = Vec::new();
    let mut others: Vec<PathBuf> = Vec::new();

    let mut stack = vec![root.to_path_buf()];
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
            if meta.permissions().mode() & 0o111 == 0 {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if name == preferred {
                matches.push(path);
            } else {
                others.push(path);
            }
        }
    }

    others.sort_by_key(|p| p.components().count());
    matches.extend(others);
    matches
}

/// UNIVERSAL SCRIPT SANITIZER
fn sanitize_hooks_generically(manifest: &mut manifest::GpkgManifest) {
    let distro_specific_commands = [
        "update-alternatives ",
        "invoke-rc.d ",
        "systemctl ",
        "debconf-",
        "dpkg-maintscript-helper ",
        "ldconfig",
    ];

    let sanitize = |script: &mut Option<String>| {
        if let Some(ref mut content) = script {
            let mut sanitized_lines = Vec::new();
            
            sanitized_lines.push("#!/bin/sh".to_string());
            sanitized_lines.push("set +e # Injected by deb2gpkg: Prevent hook failures".to_string());

            for line in content.lines() {
                if line.starts_with("#!") { continue; }

                let mut safe_line = line.to_string();
                for cmd in &distro_specific_commands {
                    if safe_line.trim().starts_with(cmd) {
                        safe_line = format!("# [d2g disabled] {}", safe_line);
                        break;
                    }
                }
                sanitized_lines.push(safe_line);
            }
            *content = sanitized_lines.join("\n");
        }
    };

    sanitize(&mut manifest.hooks.preinst);
    sanitize(&mut manifest.hooks.postinst);
    sanitize(&mut manifest.hooks.prerm);
    sanitize(&mut manifest.hooks.postrm);
}

pub fn convert_package_to_gpkg(archive_path_str: &str) -> Result<String> {
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        anyhow::bail!("Файл '{}' не найден", archive_path_str);
    }

    let pkg_name = archive_path.file_stem().unwrap_or_default().to_string_lossy();
    let staging_dir = env::temp_dir().join(format!("d2g_stage_{}", pkg_name));
    
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir).context("Failed to create staging directory")?;

    let unpacker = unpacker::get_unpacker(archive_path)?;
    let mut manifest_data = match unpacker.unpack(archive_path, &staging_dir) {
        Ok(m) => m,
        Err(unpack_err) => {
            let data_dir = staging_dir.join("data");
            match metadata_fallback::generate_fallback_manifest(archive_path, &data_dir) {
                Ok(m) => m,
                Err(fb_err) => {
                    anyhow::bail!("Распаковка не удалась: {unpack_err}. Фолбэк метаданных тоже: {fb_err}");
                }
            }
        }
    };

    let sanitizer = sanitizer::DependencySanitizer::new();
    manifest_data.dependencies = sanitizer.sanitize(manifest_data.dependencies);

    let data_dir = staging_dir.join("data");

    // Execution Links & Sandboxes
    ensure_exec_integration(&data_dir, &mut manifest_data)?;

    // BUNDLE DEPENDENCIES & PATCH ELF RPATH FOR LFS ISOLATION
    elf_linker::bundle_and_patch_elfs(&data_dir, &manifest_data.name)?;

    // UNIVERSAL SCRIPT SANITIZER
    sanitize_hooks_generically(&mut manifest_data);

    let local_logo = env::current_dir()?.join("d2g.png");
    if local_logo.exists() {
        let pixmaps_dir = data_dir.join("usr/share/pixmaps");
        fs::create_dir_all(&pixmaps_dir)?;
        let _ = fs::copy(&local_logo, pixmaps_dir.join(format!("{}.png", manifest_data.name)));
    }

    let installed_files = if data_dir.exists() {
        collect_installed_files(&data_dir)?
    } else {
        Vec::new()
    };
    manifest_data.installed_files = installed_files;

    manifest::write_manifest(&staging_dir, &manifest_data)?;

    let out_filename = format!("{}_{}_{}.gpkg", manifest_data.name, manifest_data.version, manifest_data.architecture);
    let out_path = env::current_dir()?.join(&out_filename);
    builder::build_gpkg(&staging_dir, &out_path)?;

    fs::remove_dir_all(&staging_dir).context("Failed to clean up staging directory")?;

    Ok(format!("Успешно! Пакет сохранен как:\n{}", out_filename))
}

#[tokio::main]
async fn main() {
    let ui = AppWindow::new().unwrap();
    
    let ui_handle = ui.as_weak();
    ui.on_select_file(move || {
        let ui_handle = ui_handle.clone();
        tokio::task::spawn_blocking(move || {
            // ФИЛЬТР ТОЛЬКО ДЛЯ .deb
            let picker = rfd::FileDialog::new()
                .add_filter("Debian Packages", &["deb"]);

            if let Some(file) = picker.pick_file() {
                let path = file.to_string_lossy().to_string();
                
                let temp_stage = env::temp_dir().join("d2g_preview");
                let _ = fs::remove_dir_all(&temp_stage);
                let _ = fs::create_dir_all(&temp_stage);

                let meta_result = unpacker::get_unpacker(Path::new(&path))
                    .and_then(|u| u.unpack(Path::new(&path), &temp_stage))
                    .or_else(|_| {
                        metadata_fallback::generate_fallback_manifest(Path::new(&path), &temp_stage.join("data"))
                    });

                let _ = fs::remove_dir_all(&temp_stage);

                match meta_result {
                    Ok(meta) => {
                        let deps_str = meta.dependencies.join(", ");
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.set_selected_file_path(path.into());
                                ui.set_pkg_name(meta.name.into());
                                ui.set_pkg_version(meta.version.into());
                                ui.set_pkg_arch(meta.architecture.into());
                                ui.set_pkg_deps(deps_str.into());
                                ui.set_has_metadata(true);
                                ui.set_status_message("Файл загружен. Готов к конвертации в .gpkg.".into());
                            }
                        });
                    }
                    Err(e) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_handle.upgrade() {
                                ui.set_status_message(format!("Ошибка чтения: {}", e).into());
                            }
                        });
                    }
                }
            }
        });
    });

    let ui_handle_conv = ui.as_weak();
    ui.on_convert(move || {
        let ui_handle = ui_handle_conv.clone();
        let path_str = ui_handle.unwrap().get_selected_file_path().to_string();
        
        if path_str.is_empty() { return; }

        let _ = slint::invoke_from_event_loop({
            let ui_handle = ui_handle.clone();
            move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_status_message("Сборка пакета GPKG. Пожалуйста, подождите...".into());
                }
            }
        });

        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                convert_package_to_gpkg(&path_str)
            }).await.unwrap();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    match result {
                        Ok(msg) => {
                            ui.set_status_message(msg.into());
                            ui.set_has_metadata(false);
                            ui.set_selected_file_path("".into());
                        }
                        Err(err) => ui.set_status_message(format!("Ошибка: {}", err).into()),
                    }
                }
            });
        });
    });

    let ui_handle_cancel = ui.as_weak();
    ui.on_cancel(move || {
        if let Some(ui) = ui_handle_cancel.upgrade() {
            ui.set_has_metadata(false);
            ui.set_selected_file_path("".into());
            ui.set_status_message("Отменено. Ожидание файла...".into());
        }
    });

    ui.run().unwrap();
}