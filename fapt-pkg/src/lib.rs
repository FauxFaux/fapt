#[macro_use]
extern crate error_chain;

extern crate gpgme;

extern crate mailparse;

#[macro_use]
extern crate nom;

extern crate reqwest;

extern crate tempdir;
extern crate tempfile_fast;

pub mod classic_sources_list;
pub mod commands;
mod errors;
mod fetch;
mod lists;
mod rfc822;
mod signing;

pub use errors::*;
