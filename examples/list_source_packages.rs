use failure::err_msg;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    fapt.add_sources_entry_line("deb-src http://deb.debian.org/debian sid main contrib")
        .expect("parsing static data");
    fapt_pkg::commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    fapt.walk_sections(|map| {
        let pkg = map
            .get_if_one_line("Package")
            .ok_or_else(|| err_msg("invalid Package"))?;
        println!("{}", pkg);
        Ok(())
    })?;

    Ok(())
}
