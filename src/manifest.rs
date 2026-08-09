use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct GpkgManifest {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub maintainer: String,
    pub description: String,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub exec_binary: String,
    #[serde(default)]
    pub installed_files: Vec<String>,
}

pub fn parse_control(raw: &str) -> Result<GpkgManifest> {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut current_key = String::new();

    for line in raw.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(val) = map.get_mut(&current_key) {
                val.push('\n');
                val.push_str(line.trim());
            }
        } else if let Some((key, value)) = line.split_once(':') {
            current_key = key.trim().to_lowercase();
            map.insert(current_key.clone(), value.trim().to_string());
        }
    }

    let dependencies = map
        .get("depends")
        .map(|d| parse_dependencies(d))
        .unwrap_or_default();

    let name = map.remove("package").context("Missing 'Package' field")?;

    Ok(GpkgManifest {
        exec_binary: name.clone(), // По умолчанию считаем бинарник равным имени пакета
        name,
        version: map.remove("version").context("Missing 'Version' field")?,
        architecture: map.remove("architecture").unwrap_or_else(|| "all".to_string()),
        maintainer: map.remove("maintainer").unwrap_or_default(),
        description: map.remove("description").unwrap_or_default(),
        dependencies,
        installed_files: Vec::new(), // Заполнится позже в main.rs
    })
}

fn parse_dependencies(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(idx) = s.find('(') {
                s[..idx].trim().to_string()
            } else {
                s.to_string()
            }
        })
        .collect()
}

pub fn write_manifest(staging: &Path, manifest: &GpkgManifest) -> Result<()> {
    let manifest_path = staging.join("manifest.json");
    let mut file = File::create(manifest_path).context("Failed to create manifest.json")?;
    
    let json = serde_json::to_string_pretty(manifest)?;
    file.write_all(json.as_bytes())?;
    file.flush()?; // Гарантируем сброс буфера на жесткий диск
    
    Ok(())
}