#[macro_use]
extern crate error_chain;
extern crate gpgme;
extern crate hex;
extern crate flate2;
extern crate mailparse;
extern crate md5;

#[macro_use]
extern crate nom;
extern crate reqwest;
extern crate serde_json;
extern crate sha2;
extern crate tempdir;
extern crate tempfile_fast;

use std::fmt;

mod checksum;
pub mod classic_sources_list;
pub mod commands;
mod errors;
mod fetch;
mod lists;
pub mod release;
mod rfc822;
mod signing;

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
