use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Error;
use clap::{command, Arg, Command};
use fapt::commands;
use fapt::sources_list;
use fapt::system::System;

fn main() -> Result<(), anyhow::Error> {
    let matches = command!()
        .subcommand_required(true)
        .arg(
            Arg::new("sources-list")
                .long("sources-list")
                .value_name("PREFIX")
                .help("explicitly set the sources.list search path"),
        )
        .arg(
            Arg::new("keyring")
                .long("keyring")
                .num_args(..)
                .number_of_values(1)
                .value_name("PREFIX")
                .help("explicitly add a keyring search path"),
        )
        .arg(
            Arg::new("cache-dir")
                .long("cache-dir")
                .short('c')
                .value_name("DIRECTORY")
                .help("explicitly set the cache directory"),
        )
        .arg(
            Arg::new("sources-line")
                .long("sources-line")
                .short('r')
                .value_name("LINE")
                .num_args(..)
                .number_of_values(1)
                .help(concat!(
                    "a sources.list entry",
                    " e.g. debs http://deb.debian.org/debian sid main contrib"
                )),
        )
        .arg(
            Arg::new("arch")
                .long("arch")
                .short('a')
                .value_name("ARCH")
                .num_args(..)
                .number_of_values(1)
                .help("an explicit arch (e.g. 'amd64'); the first provided will be the 'primary'"),
        )
        .arg(
            Arg::new("system-dpkg")
                .long("system-dpkg")
                .value_name("PATH")
                .default_value("/var/lib/dpkg")
                .help("dpkg database location"),
        )
        .subcommand(
            Command::new("update"), // .help("just fetch necessary data for specified sources"),
        )
        .subcommand(
            Command::new("source-ninja"), // .help("dump out all source packages as ninja"),
        )
        .get_matches();

    let mut sources_entries = Vec::with_capacity(16);
    if let Some(prefix) = matches.get_one::<&str>("sources-list") {
        for prefix in expand_dot_d(prefix)? {
            sources_entries.extend(
                sources_list::read(io::BufReader::new(fs::File::open(&prefix)?))
                    .with_context(|| anyhow!("loading sources.list: {:?}", prefix))?,
            );
        }
    }

    if let Some(lines) = matches.get_many::<&str>("sources-line") {
        for line in lines.copied() {
            let entries = sources_list::read(io::Cursor::new(line))
                .with_context(|| anyhow!("parsing command line: {:?}", line))?;

            ensure!(
                !entries.is_empty(),
                "{:?} resulted in no valid entries",
                line
            );

            sources_entries.extend(entries);
        }
    }

    let arches = match matches.get_many("arch") {
        Some(arches) => arches.copied().collect(),
        None => vec!["amd64"],
    };

    if sources_entries.is_empty() {
        bail!(concat!(
            "No sources-list entries; either specify a non-empty",
            "--sources-list, or provide some --sources-lines"
        ));
    }

    let mut system = System::cache_only()?;
    system.add_sources_entries(sources_entries.clone().into_iter());
    if let Some(keyring_paths) = matches.get_raw("keyring") {
        for keyring_path in keyring_paths {
            for path in expand_dot_d(keyring_path)? {
                system.add_keys_from(
                    fs::File::open(&path)
                        .with_context(|| anyhow!("opening key file: {:?}", path))?,
                )?;
            }
        }
    } else {
        commands::add_builtin_keys(&mut system);
    }

    system.set_arches(&arches);

    system.set_dpkg_database(matches.get_one::<&Path>("system-dpkg").unwrap());

    match matches.subcommand() {
        Some(("source-ninja", _)) => {
            commands::source_ninja(&system)?;
        }
        Some(("update", _)) => {
            system.update()?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn expand_dot_d<P: AsRef<Path>>(path: P) -> Result<Vec<PathBuf>, Error> {
    let mut ret = Vec::new();

    let path = path.as_ref();

    if path.is_dir() {
        bail!("you must provide a file, not a directory");
    }

    if path.is_file() {
        ret.push(path.to_path_buf());
    }

    let extension = path.extension();

    let mut dot_d = path.as_os_str().to_owned();
    dot_d.push(".d");

    let dot_d: PathBuf = dot_d.into();

    if dot_d.is_dir() {
        for file in fs::read_dir(dot_d)? {
            let file = file?.path();
            if file.is_file() && file.extension() == extension {
                ret.push(file);
            }
        }
    }

    if ret.is_empty() {
        bail!("no .d matches for {:?}", path);
    }

    Ok(ret)
}
