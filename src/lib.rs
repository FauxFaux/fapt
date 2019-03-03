//! A collection of tools for dealing with Debian/Ubuntu-style _Packages_ and _Repositories_.
//!
//! A _System_ can download _Package_ _Listings_ according to some _Sources Lists_.

#[macro_use]
extern crate nom;

mod checksum;
pub mod commands;
mod fetch;
mod lists;
pub mod parse;
mod release;
pub mod rfc822;
mod signing;
pub mod sources_list;
pub mod system;
