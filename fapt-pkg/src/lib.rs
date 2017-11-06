#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate nom;

extern crate reqwest;

extern crate tempfile_fast;

pub mod classic_sources_list;
mod errors;
mod fetch;
mod lists;

pub use errors::*;
