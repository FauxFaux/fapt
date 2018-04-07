#[macro_use]
extern crate error_chain;
extern crate filetime;
extern crate flate2;
extern crate gpgme;
extern crate hex;
extern crate mailparse;
extern crate md5;

#[macro_use]
extern crate nom;
extern crate reqwest;
extern crate serde_json;
extern crate sha2;
extern crate tempdir;
extern crate tempfile_fast;

#[cfg(intellij_type_hinting)]
extern crate error_chain_for_dumb_ides;

use std::fmt;

mod checksum;
pub mod classic_sources_list;
mod commands;
mod errors;
mod fetch;
mod lists;
mod release;
mod rfc822;
mod signing;

pub use commands::System;
pub use errors::*;

#[derive(Copy, Clone)]
pub struct Hashes {
    md5: [u8; 16],
    sha256: [u8; 32],
}

impl fmt::Debug for Hashes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "md5:{} sha256:{}",
            hex::encode(self.md5),
            hex::encode(self.sha256)
        )
    }
}
