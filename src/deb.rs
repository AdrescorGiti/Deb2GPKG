use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

pub fn unpack_deb(deb_path: &Path, staging: &Path) -> Result<String> {
    let file = File::open(deb_path).context("Cannot open .deb file")?;
    let mut archive = ar::Archive::new(file);

    let mut control_content = String::new();

    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result.context("Failed to read ar entry")?;
        let identifier = String::from_utf8_lossy(entry.header().identifier()).to_string();

        if identifier.starts_with("control.tar") {
            let mut tar = get_tar_decoder(&identifier, &mut entry)?;
            let control_stage = staging.join("DEBIAN");
            tar.unpack(&control_stage).context("Failed to unpack control.tar")?;

            let control_file = control_stage.join("control");
            if control_file.exists() {
                control_content = fs::read_to_string(&control_file)?;
            }

            setup_hooks(&control_stage, staging)?;
            fs::remove_dir_all(&control_stage).ok();
        } else if identifier.starts_with("data.tar") {
            let mut tar = get_tar_decoder(&identifier, &mut entry)?;
            let data_stage = staging.join("data");
            fs::create_dir_all(&data_stage)?;
            tar.unpack(&data_stage).context("Failed to unpack data.tar")?;
        }
    }

    if control_content.is_empty() {
        anyhow::bail!("control file not found in the archive");
    }

    Ok(control_content)
}

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

    Ok(tar::Archive::new(decoder))
}

fn setup_hooks(control_dir: &Path, staging: &Path) -> Result<()> {
    let hooks_dir = staging.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    for hook_name in &["preinst", "postinst", "prerm", "postrm"] {
        let hook_path = control_dir.join(hook_name);
        if hook_path.exists() {
            fs::copy(&hook_path, hooks_dir.join(hook_name))?;
        }
    }
    Ok(())
}