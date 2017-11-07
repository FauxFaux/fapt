extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate fapt_pkg;

use std::path::PathBuf;

use clap::{Arg, App, SubCommand, AppSettings};

mod errors;
use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let matches = App::new("Faux' apt")
        .setting(AppSettings::SubcommandRequired)
        .arg(
            Arg::with_name("root-dir")
                .long("root-dir")
                .value_name("DIRECTORY")
                .required(true),
        )
        .subcommand(SubCommand::with_name("update"))
        .subcommand(
            SubCommand::with_name("yaml")
                .setting(AppSettings::SubcommandRequired)
                .subcommand(SubCommand::with_name("mirrors")),
        )
        .get_matches();

    let root = PathBuf::from(matches.value_of("root-dir").expect("required"));

    match matches.subcommand() {
        ("update", Some(_)) => {
            fapt_pkg::commands::update(
                root.join("etc/apt/sources.list"),
                root.join("var/cache/fapt"),
            )?;
        }
        ("yaml", Some(matches)) => {
            match matches.subcommand() {
                ("mirrors", _) => {
                    println!(
                        "{:?}",
                        fapt_pkg::classic_sources_list::load("/etc/apt/sources.list")?
                    );
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
