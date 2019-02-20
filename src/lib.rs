#[macro_use]
extern crate nom;

mod checksum;
pub mod classic_sources_list;
pub mod commands;
mod deps;
mod fetch;
mod lists;
mod parse;
mod release;
mod signing;
mod system;

pub use crate::deps::package_list::PackageList;
pub use crate::parse::rfc822::RfcMapExt;
pub use crate::parse::types::Package;
pub use crate::system::System;
