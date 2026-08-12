use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Canonical GPKGM manifest structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub hooks: HookSet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preinst: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postinst: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postrm: Option<String>,
}

impl GpkgManifest {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
            architecture: String::new(),
            maintainer: String::new(),
            description: String::new(),
            dependencies: Vec::new(),
            exec_binary: String::new(),
            installed_files: Vec::new(),
            hooks: HookSet::default(),
            email: None,
            github: None,
        }
    }
}

// ============================================================================
// Debian `control` parser
// ============================================================================
pub fn parse_control(raw: &str) -> Result<GpkgManifest> {
    let map = parse_rfc822(raw);

    let name = map
        .get("package")
        .filter(|s| !s.is_empty())
        .context("control: missing 'Package' field")?
        .clone();

    let version = map
        .get("version")
        .filter(|s| !s.is_empty())
        .context("control: missing 'Version' field")?
        .clone();

    let dependencies = map
        .get("depends")
        .map(|d| parse_deb_dependencies(d))
        .unwrap_or_default();

    Ok(GpkgManifest {
        exec_binary: name.clone(),
        name,
        version,
        architecture: map
            .get("architecture")
            .cloned()
            .unwrap_or_else(|| "all".to_string()),
        maintainer: map.get("maintainer").cloned().unwrap_or_default(),
        description: map.get("description").cloned().unwrap_or_default(),
        dependencies,
        installed_files: Vec::new(),
        hooks: HookSet::default(),
        email: None,
        github: None,
    })
}

fn parse_deb_dependencies(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = s.split(':').next().unwrap_or(s).trim();
            if let Some(idx) = s.find('(') {
                s[..idx].trim().to_string()
            } else {
                s.to_string()
            }
        })
        .collect()
}

// ============================================================================
// Shared RFC-822 parser
// ============================================================================
fn parse_rfc822(raw: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut current_key = String::new();

    for line in raw.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(val) = map.get_mut(&current_key) {
                val.push('\n');
                val.push_str(line.trim_end());
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            current_key = key.trim().to_lowercase();
            map.insert(current_key.clone(), value.trim().to_string());
        }
    }
    map
}

// ============================================================================
// GPKGM Manifest Generator
// ============================================================================
pub fn write_manifest(staging: &Path, manifest: &GpkgManifest) -> Result<()> {
    let manifest_path = staging.join("GPKGM");
    let mut file = File::create(&manifest_path)
        .with_context(|| format!("Failed to create GPKGM at {}", manifest_path.display()))?;

    let mut content = format!(
        "name={}\nversion={}\ndescription={}\nexec={}\nmaintainer={}\narchitecture={}\n",
        manifest.name,
        manifest.version,
        manifest.description,
        manifest.exec_binary,
        manifest.maintainer,
        manifest.architecture
    );

    if !manifest.dependencies.is_empty() {
        content.push_str(&format!("dependencies={}\n", manifest.dependencies.join(", ")));
    }

    if let Some(ref email) = manifest.email {
        content.push_str(&format!("email={}\n", email));
    }

    if let Some(ref github) = manifest.github {
        content.push_str(&format!("github={}\n", github));
    }

    file.write_all(content.as_bytes())?;
    file.flush()?;
    Ok(())
}