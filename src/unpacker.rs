use anyhow::Result;
use std::path::Path;
use crate::manifest::GpkgManifest;

/// A unified trait for unpacking various upstream Linux package formats.
pub trait PackageUnpacker {
    /// Extracts the package payload into `staging/data`, hooks into `staging/hooks`,
    /// and returns the parsed or generated GpkgManifest.
    fn unpack(&self, archive_path: &Path, staging_dir: &Path) -> Result<GpkgManifest>;
}

// Example Factory to route files to the correct unpacker
pub fn get_unpacker(file_path: &Path) -> Result<Box<dyn PackageUnpacker>> {
    let file_name = file_path.to_string_lossy().to_lowercase();
    
    if file_name.ends_with(".deb") {
        Ok(Box::new(crate::deb::DebUnpacker))
    } else if file_name.ends_with(".rpm") {
        Ok(Box::new(crate::rpm::RpmUnpacker))
    } else if file_name.ends_with(".pkg.tar.zst") {
        Ok(Box::new(crate::arch::ArchUnpacker))
    } else if file_name.ends_with(".appimage") {
        Ok(Box::new(crate::appimage::AppImageUnpacker))
    } else {
        anyhow::bail!("Unsupported package format: {}", file_name)
    }
}