extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate fapt_pkg;

use std::fs;
use std::path::PathBuf;

use clap::{App, AppSettings, Arg, SubCommand};
use fapt_pkg::classic_sources_list;

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
                .help("a chroot-like place to read/write files"),
        )
        .arg(
            Arg::with_name("sources-list")
                .long("sources-list")
                .value_name("PREFIX")
                .help("explicitly set the sources.list search path"),
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
        .subcommand(
            SubCommand::with_name("update").help("just fetch necessary data for specified sources"),
        )
        .subcommand(
            SubCommand::with_name("export")
                .help("dump out all packages as json")
                .arg(Arg::with_name("format").short("f").value_name("FORMAT")),
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

    let cache_dir = cache_dir
        .ok_or("A --cache-dir is required, please set it explicitly, or provide a --root-dir")?;

    let mut sources_entries = Vec::new();
    if let Some(prefix) = sources_list_prefix {
        // TODO: sources.list.d
        sources_entries.extend(classic_sources_list::load(&prefix)
            .chain_err(|| format!("loading sources.list: {:?}", prefix))?);
    }

    if let Some(urls) = matches.values_of("release-url") {
        for url in urls {
            let octothorpe = url.find('#')
                .ok_or_else(|| format!("url must contain octothorpe: {:?}", url))?;
            let (url, extras) = url.split_at(octothorpe);
            let mut parts: Vec<&str> = extras[1..].split(',').collect();

            ensure!(
                parts.len() > 1,
                "at least one component must be specified: {:?}",
                url
            );

            let suite_codename = parts.remove(0);

            for src in &[false, true] {
                sources_entries.push(classic_sources_list::Entry {
                    src: *src,
                    url: url.to_string(),
                    suite_codename: suite_codename.to_string(),
                    components: parts.iter().map(|x| x.to_string()).collect(),
                });
            }
        }
    }

    if sources_entries.is_empty() {
        bail!(concat!(
            "No sources-list entries; either specify a non-empty",
            "--sources-list, or provide some --release-urls"
        ));
    }

    let lists_dir = cache_dir.join("lists");
    fs::create_dir_all(&lists_dir)
        .chain_err(|| format!("creating cache directory: {:?}", lists_dir))?;

    let mut system = fapt_pkg::commands::System::cache_dirs_only(lists_dir)?;
    system.add_sources_entries(sources_entries.clone().into_iter());
    system.add_keyring_paths(["/usr/share/keyrings/debian-archive-keyring.gpg"].into_iter())?;

    match matches.subcommand() {
        ("export", Some(matches)) => {
            system.export()?;
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
