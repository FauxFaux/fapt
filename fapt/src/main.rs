extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate fapt_pkg;

use clap::{Arg, App, SubCommand, AppSettings};

mod errors;
use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let matches = App::new("Faux' apt")
        .setting(AppSettings::SubcommandRequired)
        .subcommand(
            SubCommand::with_name("yaml")
                .setting(AppSettings::SubcommandRequired)
                .subcommand(SubCommand::with_name("mirrors")),
        )
        .get_matches();

    match matches.subcommand() {
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
