#[macro_use]
extern crate nom;

mod checksum;
mod classic_sources_list;
pub mod commands;
mod deps;
mod fetch;
mod lists;
mod parse;
mod release;
mod signing;
mod system;

pub use crate::system::System;
