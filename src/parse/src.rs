use std::collections::HashMap;

use failure::bail;
use failure::ensure;
use failure::err_msg;
use failure::Error;
use insideout::InsideOut;

use super::deps::parse_dep;
use super::deps::Dependency;
use super::ident::Identity;
use super::rfc822;
use super::rfc822::RfcMapExt;
use super::types;
use super::vcs;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub format: SourceFormat,

    pub binaries: Vec<SourceBinary>,
    pub files: Vec<SourceArchive>,
    pub vcs: Vec<vcs::Vcs>,

    pub directory: String,
    pub standards_version: String,

    pub build_dep: Vec<Dependency>,
    pub build_dep_arch: Vec<Dependency>,
    pub build_dep_indep: Vec<Dependency>,
    pub build_conflict: Vec<Dependency>,
    pub build_conflict_arch: Vec<Dependency>,
    pub build_conflict_indep: Vec<Dependency>,

    pub uploaders: Vec<Identity>,
}

// TODO: This is *very* similar to a ReleaseContent
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceArchive {
    name: String,
    size: u64,
    md5: crate::checksum::MD5,
    sha256: Option<crate::checksum::SHA256>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBinary {
    pub name: String,
    pub style: String,
    pub section: String,

    pub priority: types::Priority,
    pub extras: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    Original,
    Quilt3dot0,
    Native3dot0,
    Git3dot0,
}

pub fn parse_src(map: &mut rfc822::Map) -> Result<Source, Error> {
    Ok(Source {
        format: parse_format(map.take_one_line("Format")?)?,
        binaries: take_package_list(map)?,
        files: take_files(map)?,
        directory: map.take_one_line("Directory")?.to_string(),
        vcs: super::vcs::extract(map)?,
        // TODO: Option<> instead of empty string?
        standards_version: map
            .remove_one_line("Standards-Version")?
            .unwrap_or("")
            .to_string(),
        build_dep: parse_dep(&map.remove("Build-Depends").unwrap_or_else(Vec::new))?,
        build_dep_arch: parse_dep(&map.remove("Build-Depends-Arch").unwrap_or_else(Vec::new))?,
        build_dep_indep: parse_dep(&map.remove("Build-Depends-Indep").unwrap_or_else(Vec::new))?,
        build_conflict: parse_dep(&map.remove("Build-Conflicts").unwrap_or_else(Vec::new))?,
        build_conflict_arch: parse_dep(
            &map.remove("Build-Conflicts-Arch").unwrap_or_else(Vec::new),
        )?,
        build_conflict_indep: parse_dep(
            &map.remove("Build-Conflicts-Indep").unwrap_or_else(Vec::new),
        )?,
        uploaders: map
            .remove_one_line("Uploaders")?
            .map(|line| super::ident::read(line))
            .inside_out()?
            .unwrap_or_else(Vec::new),
    })
}

pub fn parse_format(string: &str) -> Result<SourceFormat, Error> {
    Ok(match string {
        "3.0 (quilt)" => SourceFormat::Quilt3dot0,
        "1.0" => SourceFormat::Original,
        "3.0 (git)" => SourceFormat::Git3dot0,
        "3.0 (native)" => SourceFormat::Native3dot0,
        other => bail!("unsupported source format: '{}'", other),
    })
}

pub fn take_package_list(map: &mut rfc822::Map) -> Result<Vec<SourceBinary>, Error> {
    let package_list = match map.remove("Package-List") {
        Some(list) => list,
        None => {
            // sigh legacy
            return Ok(map
                .take_csv("Binary")?
                .into_iter()
                // TODO: optional, instead of empty string?
                // TODO: or fallback to the values on the parent package?
                .map(|v| SourceBinary {
                    name: v.to_string(),
                    style: String::new(),
                    section: String::new(),
                    priority: super::Priority::Unknown,
                    extras: Vec::new(),
                })
                .collect());
        }
    };

    let mut binaries: HashSet<_> = map.take_csv("Binary")?.into_iter().collect();

    let mut binaries = Vec::with_capacity(package_list.len());

    for line in package_list {
        let mut parts: Vec<_> = line.split_whitespace().collect();
        ensure!(parts.len() >= 4, "package list line too short: {:?}", line);
        binaries.push(SourceBinary {
            name: parts[0].to_string(),
            style: parts[1].to_string(),
            section: parts[2].to_string(),
            priority: super::parse_priority(parts[3])?,
            extras: parts[4..].into_iter().map(|s| s.to_string()).collect(),
        });
    }

    Ok(binaries)
}

pub fn take_files(map: &mut rfc822::Map) -> Result<Vec<SourceArchive>, Error> {
    use crate::checksum::parse_md5;
    use crate::checksum::parse_sha256;
    use crate::release::take_checksums;
    let file_and_size_to_md5 =
        take_checksums(map, "Files")?.ok_or_else(|| err_msg("Files required"))?;
    let mut file_and_size_to_sha256 =
        take_checksums(map, "Checksums-Sha256")?.unwrap_or_else(HashMap::new);

    let mut archives = Vec::with_capacity(file_and_size_to_md5.len());
    for ((name, size), md5) in file_and_size_to_md5 {
        let sha256 = file_and_size_to_sha256.remove(&(name, size));
        archives.push(SourceArchive {
            name: name.to_string(),
            size,
            md5: parse_md5(md5)?,
            sha256: sha256.map(|v| parse_sha256(v)).inside_out()?,
        })
    }

    ensure!(
        file_and_size_to_sha256.is_empty(),
        "sha256sum for a file which didn't exist: {:?}",
        file_and_size_to_sha256
    );

    Ok(archives)
}
