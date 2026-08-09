#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod deb;
mod builder;
mod manifest;

use anyhow::{Context, Result};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

fn extract_deb_metadata(deb_path: &Path) -> Result<manifest::GpkgManifest> {
    let file = File::open(deb_path)?;
    let mut archive = ar::Archive::new(file);

    while let Some(entry_res) = archive.next_entry() {
        let mut entry = entry_res?;
        let id = String::from_utf8_lossy(entry.header().identifier()).to_string();
        
        if id.starts_with("control.tar") {
            let mut tar = deb::get_tar_decoder(&id, &mut entry)?;
            for file_res in tar.entries()? {
                let mut tar_file = file_res?;
                let path = tar_file.path()?.to_string_lossy().to_string();
                if path == "control" || path == "./control" {
                    let mut control_raw = String::new();
                    tar_file.read_to_string(&mut control_raw)?;
                    return manifest::parse_control(&control_raw);
                }
            }
        }
    }
    anyhow::bail!("Файл control не найден в архиве (возможно это не .deb)")
}

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

fn convert_deb_to_gpkg(deb_path_str: &str) -> Result<String> {
    let deb_path = Path::new(deb_path_str);
    if !deb_path.exists() {
        anyhow::bail!("Файл '{}' не найден", deb_path_str);
    }

    let pkg_name = deb_path.file_stem().unwrap_or_default().to_string_lossy();
    let staging_dir = env::temp_dir().join(format!("d2g_stage_{}", pkg_name));
    
    fs::remove_dir_all(&staging_dir).ok();
    fs::create_dir_all(&staging_dir)?;

    let control_raw = deb::unpack_deb(deb_path, &staging_dir)?;
    let mut manifest_data = manifest::parse_control(&control_raw)?;
    let data_dir = staging_dir.join("data");
    
    let local_logo = env::current_dir()?.join("d2g.png");
    if local_logo.exists() {
        let pixmaps_dir = data_dir.join("usr/share/pixmaps");
        fs::create_dir_all(&pixmaps_dir)?;
        fs::copy(&local_logo, pixmaps_dir.join(format!("{}.png", manifest_data.name)))?;
    }

    let installed_files = if data_dir.exists() {
        collect_installed_files(&data_dir)?
    } else {
        Vec::new()
    };
    manifest_data.installed_files = installed_files;

    manifest::write_manifest(&staging_dir, &manifest_data)?;

    let out_filename = format!("{}-{}.gpkg", manifest_data.name, manifest_data.version);
    let out_path = env::current_dir()?.join(&out_filename);
    builder::build_gpkg(&staging_dir, &out_path)?;

    fs::remove_dir_all(&staging_dir).ok();

    Ok(format!("Успешно! Пакет сохранен как:\n{}", out_filename))
}

#[tokio::main]
async fn main() {
    let ui = AppWindow::new().unwrap();
    
    let ui_handle = ui.as_weak();
    ui.on_select_file(move || {
        let ui_handle = ui_handle.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(file) = rfd::FileDialog::new().add_filter("Debian Package", &["deb"]).pick_file() {
                let path = file.to_string_lossy().to_string();
                
                match extract_deb_metadata(Path::new(&path)) {
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
                                ui.set_status_message("Файл загружен. Готов к упаковке.".into());
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
                convert_deb_to_gpkg(&path_str)
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