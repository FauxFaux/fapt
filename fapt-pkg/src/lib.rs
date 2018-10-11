extern crate chrono;
extern crate deb_version;
#[macro_use]
extern crate failure;
extern crate fapt_parse;
extern crate filetime;
extern crate flate2;
extern crate gpgme;
extern crate hex;
extern crate md5;
extern crate reqwest;
extern crate serde_json;
extern crate sha2;
extern crate tempdir;
extern crate tempfile_fast;

use std::fmt;

mod checksum;
pub mod classic_sources_list;
mod commands;
mod dep_graph;
mod fetch;
mod lists;
mod release;
mod signing;

pub use commands::System;

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
