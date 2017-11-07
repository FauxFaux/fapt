#[macro_use]
extern crate error_chain;
extern crate gpgme;
extern crate hex;
extern crate mailparse;

#[macro_use]
extern crate nom;
extern crate reqwest;
extern crate tempdir;
extern crate tempfile_fast;

use std::fmt;

pub mod classic_sources_list;
pub mod commands;
mod errors;
mod fetch;
mod lists;
mod release;
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
        use hex::ToHex;
        write!(
            f,
            "md5:{} sha256:{}",
            self.md5.to_hex(),
            self.sha256.to_hex()
        )
    }
}
