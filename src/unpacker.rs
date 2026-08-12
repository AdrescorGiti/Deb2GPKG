use anyhow::Result;
use std::path::Path;
use crate::manifest::GpkgManifest;

/// A unified trait for unpacking packages.
pub trait PackageUnpacker {
    /// Extracts the package payload into `staging/data`, hooks into `staging/hooks`,
    /// and returns the parsed or generated GpkgManifest.
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest>;
}

/// Routes files exclusively to the Debian unpacker.
pub fn get_unpacker(file_path: &Path) -> Result<Box<dyn PackageUnpacker>> {
    let file_name = file_path.to_string_lossy().to_lowercase();

    if file_name.ends_with(".deb") {
        Ok(Box::new(crate::deb::DebUnpacker))
    } else {
        anyhow::bail!("Неподдерживаемый формат. Разрешены только .deb пакеты: {}", file_path.display())
    }
}