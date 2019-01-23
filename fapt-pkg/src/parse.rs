pub mod deps;
mod ident;
pub mod rfc822;
mod src;
pub mod types;
mod vcs;

use failure::Error;

use self::types::Priority;

fn parse_priority(string: &str) -> Result<Priority, Error> {
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

fn yes_no(value: &str) -> Result<bool, Error> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => bail!("invalid value for yes/no: {:?}", other),
    }
}
