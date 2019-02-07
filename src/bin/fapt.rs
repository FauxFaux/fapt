use std::fs;
use std::path::Path;
use std::path::PathBuf;

use clap::{App, AppSettings, Arg, SubCommand};
use failure::bail;
use failure::ensure;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use fapt_pkg::classic_sources_list;

fn main() -> Result<(), failure::Error> {
    let matches = App::new("Faux' apt")
        .setting(AppSettings::SubcommandRequired)
        .arg(
            Arg::with_name("root-dir")
                .long("root-dir")
                .value_name("DIRECTORY")
                .help("a chroot-like place to read/write files"),
        )
        .arg(
            Arg::with_name("sources-list")
                .long("sources-list")
                .value_name("PREFIX")
                .help("explicitly set the sources.list search path"),
        )
        .arg(
            Arg::with_name("keyring")
                .long("keyring")
                .multiple(true)
                .number_of_values(1)
                .value_name("PREFIX")
                .help("explicitly add a keyring search path"),
        )
        .arg(
            Arg::with_name("cache-dir")
                .long("cache-dir")
                .short("c")
                .value_name("DIRECTORY")
                .help("explicitly set the cache directory"),
        )
        .arg(
            Arg::with_name("release-url")
                .long("release-url")
                .short("r")
                .value_name("URL")
                .multiple(true)
                .number_of_values(1)
                .help(concat!(
                    "a url-format sources.list entry",
                    " e.g. http://deb.debian.org/debian#sid,main,contrib,non-free"
                )),
        )
        .arg(
            Arg::with_name("arch")
                .long("arch")
                .short("a")
                .value_name("ARCH")
                .multiple(true)
                .number_of_values(1)
                .help("an explicit arch (e.g. 'amd64'); the first provided will be the 'primary'"),
        )
        .arg(
            Arg::with_name("system-dpkg")
                .long("system-dpkg")
                .value_name("PATH")
                .default_value("/var/lib/dpkg")
                .help("dpkg database location"),
        )
        .subcommand(
            SubCommand::with_name("update").help("just fetch necessary data for specified sources"),
        )
        .subcommand(SubCommand::with_name("list").help("show some packages"))
        .subcommand(
            SubCommand::with_name("export")
                .help("dump out all packages as json")
                .arg(Arg::with_name("format").short("f").value_name("FORMAT")),
        )
        .subcommand(
            SubCommand::with_name("source-ninja").help("dump out all source packages as ninja"),
        )
        .subcommand(
            SubCommand::with_name("yaml")
                .help("who knows what this could be")
                .setting(AppSettings::SubcommandRequired)
                .subcommand(SubCommand::with_name("mirrors")),
        )
        .get_matches();

    let mut cache_dir = None;
    let mut sources_list_prefix = None;

    if let Some(root) = matches.value_of("root-dir") {
        let root = PathBuf::from(root);
        sources_list_prefix = Some(root.join("etc/apt/sources.list"));
        cache_dir = Some(root.join("var/cache/fapt"));
    }

    if let Some(prefix) = matches.value_of("sources-list") {
        sources_list_prefix = Some(PathBuf::from(prefix));
    }

    if let Some(cache) = matches.value_of("cache-dir") {
        cache_dir = Some(PathBuf::from(cache));
    }

    let cache_dir = cache_dir.ok_or_else(|| {
        format_err!("A --cache-dir is required, please set it explicitly, or provide a --root-dir")
    })?;

    let mut sources_entries = Vec::new();
    if let Some(prefix) = sources_list_prefix {
        for prefix in expand_dot_d(prefix)? {
            sources_entries.extend(
                classic_sources_list::load(&prefix)
                    .with_context(|_| format_err!("loading sources.list: {:?}", prefix))?,
            );
        }
    }

    if let Some(lines) = matches.values_of("release-url") {
        for line in lines {
            let entries = classic_sources_list::read(line)
                .with_context(|_| format_err!("parsing command line: {:?}", line))?;

            ensure!(
                !entries.is_empty(),
                "{:?} resulted in no valid entries",
                line
            );

            sources_entries.extend(entries);
        }
    }

    let arches = match matches.values_of("arch") {
        Some(arches) => arches.collect(),
        None => vec!["amd64"],
    };

    if sources_entries.is_empty() {
        bail!(concat!(
            "No sources-list entries; either specify a non-empty",
            "--sources-list, or provide some --release-urls"
        ));
    }

    let mut system = fapt_pkg::System::cache_dirs_only(cache_dir.join("lists"))?;
    system.add_sources_entries(sources_entries.clone().into_iter());
    if let Some(keyrings) = matches.values_of_os("keyring") {
        for keyring in keyrings {
            system.add_keyring_paths(expand_dot_d(keyring)?.into_iter())?;
        }
    }

    system.set_arches(&arches);

    system.set_dpkg_database(matches.value_of("system-dpkg").unwrap());

    match matches.subcommand() {
        ("export", Some(_)) => {
            system.export()?;
        }
        ("list", Some(_)) => {
            system.list_installed()?;
        }
        ("source-ninja", Some(_)) => {
            system.source_ninja()?;
        }
        ("update", _) => {
            system.update()?;
        }
        ("yaml", Some(matches)) => match matches.subcommand() {
            ("mirrors", _) => {
                println!("{:?}", sources_entries,);
            }
            _ => unreachable!(),
        },
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