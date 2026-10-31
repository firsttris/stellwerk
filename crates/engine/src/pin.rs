//! The one Syncthing version agent, hub and tests run with.
//!
//! Checksums from `sha256sum.txt.asc` of the GitHub release
//! (<https://github.com/syncthing/syncthing/releases/tag/v2.1.7>). Raising the version means
//! updating all of them and running the integration tests.

pub const SYNCTHING_VERSION: &str = "v2.1.7";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
}

/// One release file for one platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// `syncthing-linux-amd64`, also the top directory inside the archive.
    pub stem: &'static str,
    pub kind: ArchiveKind,
    pub sha256: &'static str,
    /// The binary's name inside that directory.
    pub binary: &'static str,
}

impl Asset {
    /// `syncthing-linux-amd64-v2.1.7`
    pub fn dir_name(&self) -> String {
        format!("{}-{SYNCTHING_VERSION}", self.stem)
    }

    /// `syncthing-linux-amd64-v2.1.7.tar.gz`
    pub fn file_name(&self) -> String {
        let ext = match self.kind {
            ArchiveKind::TarGz => "tar.gz",
            ArchiveKind::Zip => "zip",
        };
        format!("{}.{ext}", self.dir_name())
    }

    pub fn url(&self) -> String {
        format!(
            "https://github.com/syncthing/syncthing/releases/download/{SYNCTHING_VERSION}/{}",
            self.file_name()
        )
    }
}

fn asset(stem: &'static str, kind: ArchiveKind, sha256: &'static str) -> Asset {
    let binary = match kind {
        ArchiveKind::Zip if stem.starts_with("syncthing-windows") => "syncthing.exe",
        _ => "syncthing",
    };
    Asset {
        stem,
        kind,
        sha256,
        binary,
    }
}

/// The asset for an OS and architecture as `std::env::consts` names them.
pub fn asset_for(os: &str, arch: &str) -> Option<Asset> {
    use ArchiveKind::{TarGz, Zip};
    let asset = match (os, arch) {
        ("linux", "x86_64") => asset(
            "syncthing-linux-amd64",
            TarGz,
            "2edc416e7ed136b7c858ed5f3f02a3b8316589605e54c380228ceea7acbf0142",
        ),
        ("linux", "aarch64") => asset(
            "syncthing-linux-arm64",
            TarGz,
            "48e456690cd2b97e7d26b02b5ac35f62a3b36172267b144cfeaf02c993ab955c",
        ),
        ("macos", "x86_64" | "aarch64") => asset(
            "syncthing-macos-universal",
            Zip,
            "a98fb709c79412e5586c81a1586bfb256b6e5a97b067ddf7447155bcdf6aac3f",
        ),
        ("windows", "x86_64") => asset(
            "syncthing-windows-amd64",
            Zip,
            "3ce7da770f546a8d02b65b2e7ee31f2a3b8b7c3f990ede045590cca4172ecfe1",
        ),
        ("windows", "aarch64") => asset(
            "syncthing-windows-arm64",
            Zip,
            "d535ffd611b3af223de64bc5dfc60c378881eb13d5c8ecb1431b7407094c15ff",
        ),
        _ => return None,
    };
    Some(asset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_amd64_points_at_the_release_tarball() {
        let asset = asset_for("linux", "x86_64").unwrap();
        assert_eq!(
            asset.url(),
            "https://github.com/syncthing/syncthing/releases/download/v2.1.7/syncthing-linux-amd64-v2.1.7.tar.gz"
        );
        assert_eq!(asset.binary, "syncthing");
    }

    #[test]
    fn windows_is_a_zip_with_an_exe() {
        let asset = asset_for("windows", "x86_64").unwrap();
        assert_eq!(asset.file_name(), "syncthing-windows-amd64-v2.1.7.zip");
        assert_eq!(asset.binary, "syncthing.exe");
    }

    #[test]
    fn unknown_platforms_have_no_asset() {
        assert_eq!(asset_for("freebsd", "x86_64"), None);
    }
}
