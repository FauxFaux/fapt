use failure::err_msg;
use fapt_pkg::commands;
use fapt_pkg::RfcMapExt;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    commands::add_sources_entries_from_str(
        &mut fapt,
        "deb-src http://deb.debian.org/debian sid main contrib",
    )
    .expect("parsing static data");
    commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    for list in fapt.listings()? {
        for section in fapt.open_listing(&list)? {
            let section = section?;
            let map = section.as_map()?;
            let pkg = map
                .get_if_one_line("Package")
                .ok_or_else(|| err_msg("invalid Package"))?;
            println!("{}", pkg);
        }
    }

    Ok(())
}
