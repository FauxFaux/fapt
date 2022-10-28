use anyhow::bail;
use anyhow::Error;
use insideout::InsideOut;

use self::rfc822::RfcMapExt;
use super::deps::parse_dep;
use super::deps::Dependency;
use super::pkg;
use crate::rfc822;

/// Binary package specific fields.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Binary {
    // "File" is missing in e.g. dpkg/status, but never in Packages as far as I've seen
    pub file: Option<pkg::File>,

    pub essential: bool,
    pub build_essential: bool,

    pub installed_size: u64,

    pub description: String,
    pub source: Option<String>,
    pub status: Option<String>,

    pub depends: Vec<Dependency>,
    pub recommends: Vec<Dependency>,
    pub suggests: Vec<Dependency>,
    pub enhances: Vec<Dependency>,
    pub pre_depends: Vec<Dependency>,

    pub breaks: Vec<Dependency>,
    pub conflicts: Vec<Dependency>,
    pub replaces: Vec<Dependency>,

    pub provides: Vec<Dependency>,
}

pub(super) fn parse_bin(it: &mut rfc822::Map) -> Result<Binary, Error> {
    let file = if it.contains_key("Filename") {
        Some(super::pkg::File {
            name: it.remove_value("Filename").one_line_req()?.to_string(),
            size: it.remove_value("Size").one_line_req()?.parse()?,
            md5: it.remove_value("MD5sum").one_line_owned()?,
            sha1: it.remove_value("SHA1").one_line_req()?.to_string(),
            sha256: it.remove_value("SHA256").one_line_req()?.to_string(),
            sha512: String::new(),
        })
    } else {
        None
    };

    // TODO: this is missing in a couple of cases in dpkg/status; pretty crap
    let installed_size = it
        .remove_value("Installed-Size")
        .one_line()?
        .map(|v| v.parse())
        .inside_out()?
        .unwrap_or(0);

    let essential = it
        .remove_value("Essential")
        .one_line()?
        .map(|line| yes_no(line))
        .inside_out()?
        .unwrap_or(false);

    let build_essential = it
        .remove_value("Build-Essential")
        .one_line()?
        .map(|line| yes_no(line))
        .inside_out()?
        .unwrap_or(false);

    Ok(Binary {
        file,
        essential,
        build_essential,
        installed_size,
        description: it.remove_value("Description").joined_lines_req()?,
        source: it.remove_value("Source").one_line_owned()?,
        status: it.remove_value("Status").one_line_owned()?,
        depends: parse_dep(&it.remove("Depends").unwrap_or_else(Vec::new))?,
        recommends: parse_dep(&it.remove("Recommends").unwrap_or_else(Vec::new))?,
        suggests: parse_dep(&it.remove("Suggests").unwrap_or_else(Vec::new))?,
        enhances: parse_dep(&it.remove("Enhances").unwrap_or_else(Vec::new))?,
        pre_depends: parse_dep(&it.remove("Pre-Depends").unwrap_or_else(Vec::new))?,
        breaks: parse_dep(&it.remove("Breaks").unwrap_or_else(Vec::new))?,
        conflicts: parse_dep(&it.remove("Conflicts").unwrap_or_else(Vec::new))?,
        replaces: parse_dep(&it.remove("Replaces").unwrap_or_else(Vec::new))?,
        provides: parse_dep(&it.remove("Provides").unwrap_or_else(Vec::new))?,
    })
}

fn yes_no(value: &str) -> Result<bool, Error> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => bail!("invalid value for yes/no: {:?}", other),
    }
}
