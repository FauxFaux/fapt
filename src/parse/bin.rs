use failure::Error;
use insideout::InsideOut;

use super::deps::parse_dep;
use super::deps::Dependency;
use super::rfc822;
use super::rfc822::RfcMapExt;
use super::types;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Binary {
    // "File" is missing in e.g. dpkg/status, but never in Packages as far as I've seen
    pub file: Option<types::File>,

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

pub fn parse_bin(it: &mut rfc822::Map) -> Result<Binary, Error> {
    // TODO: clearly `parse_file` is supposed to be called here somewhere
    let file = None;

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
        .map(|line| super::yes_no(line))
        .inside_out()?
        .unwrap_or(false);

    let build_essential = it
        .remove_value("Build-Essential")
        .one_line()?
        .map(|line| super::yes_no(line))
        .inside_out()?
        .unwrap_or(false);

    Ok(Binary {
        file,
        essential,
        build_essential,
        installed_size,
        description: rfc822::joined(&it.take_err("Description")?),
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
