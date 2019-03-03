use std::collections::HashMap;
use std::collections::HashSet;

use failure::bail;
use failure::err_msg;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use insideout::InsideOut;

use super::arch;
use super::bin;
use super::ident;
use super::src;
use crate::rfc822;
use crate::rfc822::RfcMapExt;

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

impl Package {
    pub fn parse(map: &mut rfc822::Map) -> Result<Package, Error> {
        let name = map
            .get_value("Package")
            .one_line_req()
            .with_context(|_| format_err!("no Package: {:?}", map))?
            .to_string();

        let style = if map.contains_key("Binary") {
            // Binary indicates that it's a source package *producing* that binary
            PackageType::Source(
                src::parse_src(map).with_context(|_| format_err!("source fields in {:?}", name))?,
            )
        } else {
            PackageType::Binary(
                bin::parse_bin(map).with_context(|_| format_err!("binary fields in {:?}", name))?,
            )
        };

        Ok(parse_pkg(map, style).with_context(|_| format_err!("shared fields in {:?}", name))?)
    }

    pub fn as_src(&self) -> Option<&src::Source> {
        match &self.style {
            PackageType::Source(src) => Some(src),
            _ => None,
        }
    }

    pub fn as_bin(&self) -> Option<&bin::Binary> {
        match &self.style {
            PackageType::Binary(bin) => Some(bin),
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
        .collect::<Result<HashSet<arch::Arch>, Error>>()
        .with_context(|_| err_msg("reading Architecture"))?;

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
            .map(|p| parse_priority(p))
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

pub fn parse_priority(string: &str) -> Result<Priority, Error> {
    Ok(match string {
        "required" => Priority::Required,
        "important" => Priority::Important,
        "standard" => Priority::Standard,
        "optional" => Priority::Optional,
        "extra" => Priority::Extra,
        "source" => Priority::Source,
        "unknown" => Priority::Unknown,
        other => bail!("unsupported priority: '{}'", other),
    })
}
