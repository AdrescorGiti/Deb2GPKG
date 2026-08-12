use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use crate::manifest::{self, GpkgManifest, HookSet};
use crate::unpacker::PackageUnpacker;

pub struct DebUnpacker;

impl PackageUnpacker for DebUnpacker {
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest> {
        let (control_raw, hooks) = unpack_deb(archive_path, staging_dir)?;
        let mut manifest = manifest::parse_control(&control_raw)?;
        manifest.hooks = hooks;
        Ok(manifest)
    }
}

/// Extract a `.deb` ar archive into `staging/data` (payload) and `staging/hooks`
/// (lifecycle scripts). Returns the raw `control` file text and the parsed hooks.
pub fn unpack_deb(deb_path: &Path, staging: &Path) -> Result<(String, HookSet)> {
    let file = File::open(deb_path).context("Cannot open .deb file")?;
    let mut archive = ar::Archive::new(file);

    let mut control_content = String::new();
    let mut hooks = HookSet::default();

    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result.context("Failed to read ar entry")?;
        let identifier = String::from_utf8_lossy(entry.header().identifier()).to_string();

        if identifier.starts_with("control.tar") {
            let mut tar = get_tar_decoder(&identifier, &mut entry)?;
            // Preserve mode/uid/gid/symlinks — critical for 1:1 parity.
            tar.set_preserve_permissions(true);
            let control_stage = staging.join("DEBIAN");
            fs::create_dir_all(&control_stage)?;
            
            tar.unpack(&control_stage).context("Failed to unpack control.tar")?;

            let control_file = control_stage.join("control");
            if control_file.exists() {
                control_content = fs::read_to_string(&control_file)?;
            }

            hooks = collect_hooks(&control_stage, staging)?;
            fs::remove_dir_all(&control_stage).ok();
        } else if identifier.starts_with("data.tar") {
            let mut tar = get_tar_decoder(&identifier, &mut entry)?;
            tar.set_preserve_permissions(true);
            let data_stage = staging.join("data");
            fs::create_dir_all(&data_stage)?;
            tar.unpack(&data_stage).context("Failed to unpack data.tar")?;
        }
    }

    if control_content.is_empty() {
        anyhow::bail!("control file not found in the archive");
    }

    Ok((control_content, hooks))
}

/// Build the decoder stack for a `control.tar.*` / `data.tar.*` member of a `.deb`.
pub fn get_tar_decoder<'a>(
    identifier: &str,
    reader: impl Read + 'a,
) -> Result<tar::Archive<Box<dyn Read + 'a>>> {
    let decoder: Box<dyn Read> = if identifier.ends_with(".gz") {
        Box::new(flate2::read::GzDecoder::new(reader))
    } else if identifier.ends_with(".xz") {
        Box::new(xz2::read::XzDecoder::new(reader))
    } else if identifier.ends_with(".zst") {
        Box::new(zstd::stream::Decoder::new(reader)?)
    } else if identifier.ends_with(".tar") {
        Box::new(reader)
    } else {
        anyhow::bail!("Unsupported compression format: {}", identifier);
    };

    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    Ok(archive)
}

/// Read the four standard Debian maintainer scripts into a `HookSet` and also
/// copy them into `staging/hooks` so the builder can ship them verbatim.
fn collect_hooks(control_dir: &Path, staging: &Path) -> Result<HookSet> {
    let hooks_dir = staging.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let mut set = HookSet::default();

    for (name, slot) in [
        ("preinst", &mut set.preinst),
        ("postinst", &mut set.postinst),
        ("prerm", &mut set.prerm),
        ("postrm", &mut set.postrm),
    ] {
        let hook_path = control_dir.join(name);
        if hook_path.exists() {
            let content = fs::read(&hook_path)
                .with_context(|| format!("Failed to read hook {name}"))?;
            fs::copy(&hook_path, hooks_dir.join(name))?;
            *slot = Some(String::from_utf8_lossy(&content).into_owned());
        }
    }

    Ok(set)
}