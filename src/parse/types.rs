use std::collections::HashMap;
use std::collections::HashSet;

use failure::Error;
use insideout::InsideOut;

use super::arch;
use super::bin;
use super::ident;
use super::rfc822;
use super::rfc822::RfcMapExt;
use super::src;

/// The parsed top-level types for package
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageType {
    Source(src::Source),
    Binary(bin::Binary),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub priority: Priority,
    pub arches: arch::Arches,
    pub section: String,

    pub maintainer: Vec<ident::Identity>,
    pub original_maintainer: Vec<ident::Identity>,

    pub homepage: Option<String>,

    pub unparsed: HashMap<String, Vec<String>>,

    pub style: PackageType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub name: String,
    pub size: u64,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
}

/// https://www.debian.org/doc/debian-policy/#priorities
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Priority {
    Unknown,
    Required,
    Important,
    Standard,
    Optional,
    Extra,
    Source,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Unknown
    }
}

pub struct Description {
    pub locale: String,
    pub value: String,
}

impl Package {
    pub fn parse(map: &mut rfc822::Map) -> Result<Package, Error> {
        let style = if map.contains_key("Binary") {
            // Binary indicates that it's a source package *producing* that binary
            PackageType::Source(src::parse_src(map)?)
        } else {
            PackageType::Binary(bin::parse_bin(map)?)
        };

        parse_pkg(map, style)
    }

    pub fn bin(&self) -> Option<&bin::Binary> {
        match &self.style {
            PackageType::Binary(bin) => Some(&bin),
            _ => None,
        }
    }
}

fn parse_pkg(map: &mut rfc822::Map, style: PackageType) -> Result<Package, Error> {
    let arches = map
        .remove_value("Architecture")
        .one_line_req()?
        // TODO: alternate splitting rules?
        .split_whitespace()
        .map(|s| s.parse())
        .collect::<Result<HashSet<arch::Arch>, Error>>()?;

    let original_maintainer = map
        .remove_value("Original-Maintainer")
        .one_line()?
        .map(|line| super::ident::read(line))
        .inside_out()?
        .unwrap_or_else(Vec::new);

    Ok(Package {
        name: map.remove_value("Package").one_line_req()?.to_string(),
        version: map.remove_value("Version").one_line_req()?.to_string(),
        priority: map
            .remove_value("Priority")
            .one_line()?
            .map(|p| super::parse_priority(p))
            .inside_out()?
            .unwrap_or(Priority::Unknown),
        arches,
        section: map.remove_value("Section").one_line_req()?.to_string(),
        maintainer: super::ident::read(map.remove_value("Maintainer").one_line_req()?)?,
        original_maintainer,
        homepage: map.remove_value("Homepage").one_line_owned()?,
        style,
        unparsed: map
            .into_iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.into_iter().map(|v| v.to_string()).collect(),
                )
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::PackageType;

    const SOURCE_BLOCK_ALIEN: &str = r#"Package: alien-arena
Binary: alien-arena, alien-arena-server
Version: 7.66+dfsg-5
Maintainer: Debian Games Team <pkg-games-devel@lists.alioth.debian.org>
Uploaders: Michael Gilbert <mgilbert@debian.org>, Barry deFreese <bddebian@comcast.net>
Build-Depends: debhelper (>= 10), sharutils, libglu1-mesa-dev, libgl1-mesa-dev, libjpeg-dev, libpng-dev, libxxf86vm-dev, libxxf86dga-dev, libxext-dev, libx11-dev, libcurl4-gnutls-dev, libopenal-dev, libvorbis-dev, libfreetype6-dev, pkg-config
Architecture: any
Standards-Version: 4.0.1
Format: 3.0 (quilt)
Files:
 f26e5a6a298163277318a720b77a3b58 2291 alien-arena_7.66+dfsg-5.dsc
 af12838d2346b05a6e043141ceb40c49 1767600 alien-arena_7.66+dfsg.orig.tar.gz
 d806e404397c6338eae0d6470b4e8723 13844 alien-arena_7.66+dfsg-5.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/games-team/alien-arena
Vcs-Git: https://salsa.debian.org/games-team/alien-arena.git
Checksums-Sha256:
 85eabee2877db5e070cd6549078ece3e5b4bc35a3a33ff8987d06fbb9732cd6e 2291 alien-arena_7.66+dfsg-5.dsc
 d4d173aba65fbdbf338e4fbdcb04a888e0cd3790e6de72597ba74b0bef42c14b 1767600 alien-arena_7.66+dfsg.orig.tar.gz
 6e90eabd98ac9c98ebe55b064ceb427101a3d4d4ff0b8aa4a2cea28052ec34c1 13844 alien-arena_7.66+dfsg-5.debian.tar.xz
Homepage: http://red.planetarena.org
Package-List:
 alien-arena deb contrib/games optional arch=any
 alien-arena-server deb contrib/games optional arch=any
Directory: pool/contrib/a/alien-arena
Priority: source
Section: contrib/games
"#;

    #[test]
    fn parse_alien() {
        let pkg = super::Package::parse(
            &mut super::rfc822::scan(SOURCE_BLOCK_ALIEN)
                .collect_to_map()
                .unwrap(),
        )
        .unwrap();

        assert_eq!("7.66+dfsg-5", pkg.version);

        let src = match pkg.style {
            PackageType::Source(s) => s,
            other => panic!("bad type: {:?}", other),
        };

        assert_eq!(
            vec!["alien-arena", "alien-arena-server"],
            src.binaries
                .into_iter()
                .map(|b| b.name)
                .collect::<Vec<String>>()
        );
        assert_eq!(HashMap::new(), pkg.unparsed);
    }

    const SOURCE_OLD_STABLE: &str = r#"Package: aa3d
Binary: aa3d
Version: 1.0-8
Maintainer: Uwe Hermann <uwe@debian.org>
Build-Depends: cdbs, debhelper (>= 5)
Architecture: any
Standards-Version: 3.8.0
Format: 1.0
Files:
 398d64179a3b8ffb9ac54e9f5e42f08e 951 aa3d_1.0-8.dsc
 e9bb49ac09381d96d31d44d3b7e97e8a 10198 aa3d_1.0.orig.tar.gz
 8db26e00404f2ac86e8c906680144b39 5363 aa3d_1.0-8.diff.gz
Checksums-Sha256:
 0bf2cda9b6413a545abe4d7f56a0db14b000d6d5f7d0bd37546ba649d4e7e9e7 951 aa3d_1.0-8.dsc
 944621bd7bf177178a7ecb98b274230744c5e2ae6aa0996ed83332a2fb96e6ee 10198 aa3d_1.0.orig.tar.gz
 de196bb8101f73333d1ed9a6724d7da107e53c1e3701dda603d30bbc6292a484 5363 aa3d_1.0-8.diff.gz
Homepage: http://aa-project.sourceforge.net/aa3d/
Directory: pool/main/a/aa3d
Priority: source
Section: graphics
"#;

    #[test]
    fn no_package_list() {
        super::Package::parse(
            &mut super::rfc822::scan(SOURCE_OLD_STABLE)
                .collect_to_map()
                .unwrap(),
        )
        .unwrap();
    }

    const PROVIDES_EXAMPLE: &str = r#"Package: python3-cffi-backend
Status: install ok installed
Priority: optional
Section: python
Installed-Size: 190
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Source: python-cffi
Version: 1.11.5-1
Replaces: python3-cffi (<< 1)
Provides: python3-cffi-backend-api-9729, python3-cffi-backend-api-max (= 10495), python3-cffi-backend-api-min (= 9729)
Depends: python3 (<< 3.7), python3 (>= 3.6~), python3:any (>= 3.1~), libc6 (>= 2.14), libffi6 (>= 3.0.4)
Breaks: python3-cffi (<< 1)
Description: Foreign Function Interface for Python 3 calling C code - runtime
 Convenient and reliable way of calling C code from Python 3.
 .
 The aim of this project is to provide a convenient and reliable way of calling
 C code from Python. It keeps Python logic in Python, and minimises the C
 required. It is able to work at either the C API or ABI level, unlike most
 other approaches, that only support the ABI level.
 .
 This package contains the runtime support for pre-built cffi modules.
Original-Maintainer: Debian Python Modules Team <python-modules-team@lists.alioth.debian.org>
Homepage: http://cffi.readthedocs.org/
"#;

    #[test]
    fn parse_provides() {
        let p = super::Package::parse(
            &mut super::rfc822::scan(PROVIDES_EXAMPLE)
                .collect_to_map()
                .unwrap(),
        )
        .unwrap();
        assert_eq!("python3-cffi-backend", p.name.as_str());
        let bin = match p.style {
            PackageType::Binary(bin) => bin,
            _ => panic!("wrong type!"),
        };
        assert_eq!(3, bin.provides.len());
        assert_eq!(HashMap::new(), p.unparsed);
    }
}
