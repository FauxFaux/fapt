use std::collections::HashMap;

use failure::bail;
use failure::ensure;
use failure::err_msg;
use failure::Error;
use insideout::InsideOut;

use super::rfc822;
use super::types::RfcMapExt;
use super::types::SourceBinary;
use super::types::SourceFormat;
use std::collections::HashSet;

// TODO: This is *very* similar to a ReleaseContent
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceArchive {
    name: String,
    size: u64,
    md5: crate::checksum::MD5,
    sha256: Option<crate::checksum::SHA256>,
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
                .take_err("Binary")?
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

    let mut binaries: HashSet<_> = map.take_one_line("Binary")?.split_whitespace().collect();

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
