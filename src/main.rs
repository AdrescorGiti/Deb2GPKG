#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod builder;
mod manifest;
mod deb;
mod unpacker;
mod sanitizer;
mod metadata_fallback;

// Заглушки под новые модули (создашь их по мере готовности)
mod rpm;
mod arch;
mod appimage;

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;

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

// === ПОСЛЕДНИЙ БЛОК КОДА ИЗ ПУНКТА 4 ВСТАВЛЯЕТСЯ СЮДА ===
pub fn convert_package_to_gpkg(archive_path_str: &str) -> Result<String> {
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        anyhow::bail!("Файл '{}' не найден", archive_path_str);
    }

    let pkg_name = archive_path.file_stem().unwrap_or_default().to_string_lossy();
    let staging_dir = env::temp_dir().join(format!("d2g_stage_{}", pkg_name));
    
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir).context("Failed to create staging directory")?;

    // 1. Распаковка и метаданные через универсальный Trait
    let unpacker = unpacker::get_unpacker(archive_path)?;
    let mut manifest_data = unpacker.unpack(archive_path, &staging_dir)
        .unwrap_or_else(|_| metadata_fallback::generate_fallback_manifest(archive_path, &staging_dir.join("data")).unwrap());

    // 2. Очистка зависимостей от дистро-специфичного мусора
    let sanitizer = sanitizer::DependencySanitizer::new();
    manifest_data.dependencies = sanitizer.sanitize(manifest_data.dependencies);

    // 3. Подкладывание логотипа D2G
    let data_dir = staging_dir.join("data");
    let local_logo = env::current_dir()?.join("d2g.png");
    if local_logo.exists() {
        let pixmaps_dir = data_dir.join("usr/share/pixmaps");
        fs::create_dir_all(&pixmaps_dir)?;
        fs::copy(&local_logo, pixmaps_dir.join(format!("{}.png", manifest_data.name)))?;
    }

    // 4. Сбор списка установленных файлов
    let installed_files = if data_dir.exists() {
        collect_installed_files(&data_dir)?
    } else {
        Vec::new()
    };
    manifest_data.installed_files = installed_files;

    // 5. Запись манифеста
    manifest::write_manifest(&staging_dir, &manifest_data)?;

    // 6. Сборка итогового .gpkg пакета
    let out_filename = format!("{}_{}_{}.gpkg", manifest_data.name, manifest_data.version, manifest_data.architecture);
    let out_path = env::current_dir()?.join(&out_filename);
    builder::build_gpkg(&staging_dir, &out_path)?;

    // Очистка временного каталога
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
            // Расширили фильтр: теперь разрешаем выбырать .deb, .rpm, .pkg.tar.zst и .AppImage
            let picker = rfd::FileDialog::new()
                .add_filter("Supported Packages", &["deb", "rpm", "zst", "AppImage", "appimage"]);

            if let Some(file) = picker.pick_file() {
                let path = file.to_string_lossy().to_string();
                
                // Пробуем быстро прочитать метаданные или генерируем фолбэк для превью в GUI
                let temp_stage = env::temp_dir().join("d2g_preview");
                let _ = fs::remove_dir_all(&temp_stage);
                let _ = fs::create_dir_all(&temp_stage);

                let meta_result = unpacker::get_unpacker(Path::new(&path))
                    .and_then(|u| u.unpack(Path::new(&path), &temp_stage))
                    .or_else(|_| metadata_fallback::generate_fallback_manifest(Path::new(&path), &temp_stage.join("data")));

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