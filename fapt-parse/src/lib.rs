#[macro_use]
extern crate bitflags;
extern crate deb_version;

#[macro_use]
extern crate error_chain;

extern crate mailparse;
extern crate md5;

#[macro_use]
extern crate nom;

pub mod deps;
mod errors;
mod ident;
pub mod rfc822;
mod src;
pub mod types;
mod vcs;

pub use errors::*;
use types::Priority;

fn parse_priority(string: &str) -> Result<Priority> {
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

fn yes_no(value: &str) -> Result<bool> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => bail!("invalid value for yes/no: {:?}", other),
    }
}
