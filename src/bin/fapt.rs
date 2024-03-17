use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{anyhow, bail, ensure, Context, Error, Result};
use clap::{command, Parser, Subcommand};
use fapt::commands;
use fapt::sources_list;
use fapt::system::System;

#[derive(Parser)]
struct Cli {
    /// explicitly set the sources.list search path
    #[clap(long, value_name = "PREFIX")]
    sources_list: Option<PathBuf>,
    /// explicitly add a keyring search path
    #[clap(long, value_name = "PREFIX", number_of_values = 1)]
    keyring: Option<Vec<PathBuf>>,
    /// explicitly set the cache directory
    #[clap(short = 'c', long, value_name = "DIRECTORY")]
    cache_dir: Option<PathBuf>,
    /// a sources.list entry, e.g. 'debs http://deb.debian.org/debian sid main contrib'
    #[clap(short = 'r', long, value_name = "LINE", num_args = 1)]
    sources_line: Option<Vec<String>>,
    /// an explicit arch (e.g. 'amd64'); the first provided will be the 'primary'
    arch: Option<Vec<String>>,
    /// dpkg database location
    #[clap(long, value_name = "PATH", default_value = "/var/lib/dpkg")]
    system_dpkg: PathBuf,
    #[command(subcommand)]
    subcommand: Sub,
}

#[derive(Subcommand)]
enum Sub {
    Update,
    SourceNinja,
}

fn main() -> Result<()> {
    let matches: Cli = Cli::parse();

    let mut sources_entries = Vec::with_capacity(16);
    if let Some(prefix) = matches.sources_list {
        for prefix in expand_dot_d(prefix)? {
            sources_entries.extend(
                sources_list::read(io::BufReader::new(fs::File::open(&prefix)?))
                    .with_context(|| anyhow!("loading sources.list: {:?}", prefix))?,
            );
        }
    }

    if let Some(lines) = matches.sources_line {
        for line in lines.iter() {
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

    let arches = match matches.arch {
        Some(arches) => arches.iter().cloned().collect(),
        None => vec!["amd64".to_string()],
    };

    if sources_entries.is_empty() {
        bail!(concat!(
            "No sources-list entries; either specify a non-empty",
            "--sources-list, or provide some --sources-lines"
        ));
    }

    let mut system = System::cache_only()?;
    system.add_sources_entries(sources_entries.clone().into_iter());
    if let Some(keyring_paths) = matches.keyring {
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

    system.set_dpkg_database(&matches.system_dpkg);

    match matches.subcommand {
        Sub::SourceNinja => {
            commands::source_ninja(&system)?;
        }
        Sub::Update => {
            system.update()?;
        }
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
