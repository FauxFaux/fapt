#[macro_use]
extern crate nom;

use std::fmt;

mod checksum;
pub mod classic_sources_list;
mod commands;
mod deps;
mod fetch;
mod lists;
mod parse;
mod release;
mod signing;

pub use crate::commands::System;

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
