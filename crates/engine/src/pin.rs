//! The one Syncthing version agent, hub and tests run with.
//!
//! Checksums from `sha256sum.txt.asc` of the GitHub release
//! (<https://github.com/syncthing/syncthing/releases/tag/v2.1.6>). Raising the version means
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
    /// `syncthing-linux-amd64-v2.1.6`
    pub fn dir_name(&self) -> String {
        format!("{}-{SYNCTHING_VERSION}", self.stem)
    }

    /// `syncthing-linux-amd64-v2.1.6.tar.gz`
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
            "524ef4e1df1850b719e2378c8369956f4e5f2ab9a46fdf7f45f2b925012147aa",
        ),
        ("linux", "aarch64") => asset(
            "syncthing-linux-arm64",
            TarGz,
            "f054de02462d5d58bd126908f2f92b63aba74f162a5be85e50e96d656e7db47d",
        ),
        ("macos", "x86_64" | "aarch64") => asset(
            "syncthing-macos-universal",
            Zip,
            "83d2898de5607d12c25d66856b1e3aa40326814e8ded6af98ab8ba68c2a545bc",
        ),
        ("windows", "x86_64") => asset(
            "syncthing-windows-amd64",
            Zip,
            "589098269cabb9a5a1703a934305b4153609d7623e978e1c94f0f49d77ae5e9f",
        ),
        ("windows", "aarch64") => asset(
            "syncthing-windows-arm64",
            Zip,
            "ddb25a3cf556157e329263bd3331d889ea7bb952b85a5f530320175105bab072",
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
            "https://github.com/syncthing/syncthing/releases/download/v2.1.6/syncthing-linux-amd64-v2.1.6.tar.gz"
        );
        assert_eq!(asset.binary, "syncthing");
    }

    #[test]
    fn windows_is_a_zip_with_an_exe() {
        let asset = asset_for("windows", "x86_64").unwrap();
        assert_eq!(asset.file_name(), "syncthing-windows-amd64-v2.1.6.zip");
        assert_eq!(asset.binary, "syncthing.exe");
    }

    #[test]
    fn unknown_platforms_have_no_asset() {
        assert_eq!(asset_for("freebsd", "x86_64"), None);
    }
}
