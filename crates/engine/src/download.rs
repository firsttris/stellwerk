//! Fetches the pinned Syncthing release once into a cache directory.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result, io_at};
use crate::pin::{ArchiveKind, Asset, asset_for};

/// Files taken out of the archive: the binary and Syncthing's license (MPL-2.0), which goes
/// along wherever the binary goes.
const LICENSE: &str = "LICENSE.txt";

/// Path of the pinned Syncthing binary for this platform, downloaded and checked on first use.
pub async fn ensure_syncthing(cache_dir: &Path) -> Result<PathBuf> {
    let asset = asset_for(std::env::consts::OS, std::env::consts::ARCH).ok_or(
        Error::UnsupportedPlatform {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        },
    )?;
    let dir = cache_dir.join(asset.dir_name());
    let binary = dir.join(asset.binary);
    if binary.is_file() {
        return Ok(binary);
    }

    tracing::info!(url = asset.url(), "downloading Syncthing");
    let archive = reqwest::get(asset.url())
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    check_sha256(&asset.file_name(), &archive, asset.sha256)?;

    let files = tokio::task::spawn_blocking(move || extract(&asset, &archive))
        .await
        .map_err(|err| Error::Archive(err.to_string()))??;
    std::fs::create_dir_all(&dir).map_err(io_at(&dir))?;
    for (name, contents) in files {
        write_atomically(&dir.join(name), &contents)?;
    }
    make_executable(&binary)?;
    Ok(binary)
}

/// Fails unless `data` has the expected SHA-256 (lower-case hex).
pub fn check_sha256(asset: &str, data: &[u8], expected: &str) -> Result<()> {
    let actual = hex(&Sha256::digest(data));
    if actual == expected {
        Ok(())
    } else {
        Err(Error::Checksum {
            asset: asset.to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The binary and the license from the archive, by file name.
fn extract(asset: &Asset, archive: &[u8]) -> Result<Vec<(&'static str, Vec<u8>)>> {
    let wanted = [asset.binary, LICENSE];
    let prefix = asset.dir_name();
    let mut found = match asset.kind {
        ArchiveKind::TarGz => extract_tar_gz(archive, &prefix, &wanted)?,
        ArchiveKind::Zip => extract_zip(archive, &prefix, &wanted)?,
    };
    wanted
        .iter()
        .map(|name| {
            let index = found
                .iter()
                .position(|(found, _)| found == name)
                .ok_or_else(|| Error::MissingEntry {
                    asset: asset.file_name(),
                    entry: format!("{prefix}/{name}"),
                })?;
            Ok(found.swap_remove(index))
        })
        .collect()
}

fn extract_tar_gz(
    archive: &[u8],
    prefix: &str,
    wanted: &[&'static str],
) -> Result<Vec<(&'static str, Vec<u8>)>> {
    let archive_err = |err: std::io::Error| Error::Archive(err.to_string());
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut found = Vec::new();
    for entry in tar.entries().map_err(archive_err)? {
        let mut entry = entry.map_err(archive_err)?;
        let path = entry
            .path()
            .map_err(archive_err)?
            .to_string_lossy()
            .into_owned();
        if let Some(name) = wanted_name(&path, prefix, wanted) {
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).map_err(archive_err)?;
            found.push((name, contents));
        }
    }
    Ok(found)
}

fn extract_zip(
    archive: &[u8],
    prefix: &str,
    wanted: &[&'static str],
) -> Result<Vec<(&'static str, Vec<u8>)>> {
    let archive_err = |err: zip::result::ZipError| Error::Archive(err.to_string());
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(archive_err)?;
    let mut found = Vec::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(archive_err)?;
        if let Some(name) = wanted_name(file.name(), prefix, wanted) {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|err| Error::Archive(err.to_string()))?;
            found.push((name, contents));
        }
    }
    Ok(found)
}

fn wanted_name(path: &str, prefix: &str, wanted: &[&'static str]) -> Option<&'static str> {
    let name = path.strip_prefix(prefix)?.strip_prefix('/')?;
    wanted.iter().copied().find(|w| *w == name)
}

/// Writes next to the target and renames, so a parallel test never sees half a binary.
fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("part-{}", std::process::id()));
    std::fs::write(&tmp, contents).map_err(io_at(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io_at(path))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(io_at(path))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_of_known_input() {
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(check_sha256("empty", b"", empty).is_ok());
        assert!(matches!(
            check_sha256("x", b"x", empty),
            Err(Error::Checksum { .. })
        ));
    }

    #[test]
    fn only_entries_directly_in_the_release_directory_count() {
        let wanted = ["syncthing", LICENSE];
        let dir = "syncthing-linux-amd64-v2.1.6";
        assert_eq!(
            wanted_name("syncthing-linux-amd64-v2.1.6/syncthing", dir, &wanted),
            Some("syncthing")
        );
        assert_eq!(
            wanted_name("syncthing-linux-amd64-v2.1.6/etc/syncthing", dir, &wanted),
            None
        );
        assert_eq!(wanted_name("other/syncthing", dir, &wanted), None);
    }
}
