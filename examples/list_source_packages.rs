#[macro_use]
extern crate failure;

extern crate fapt_pkg;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    fapt.add_sources_entry_line("deb-src http://deb.debian.org/debian sid main contrib")
        .expect("parsing static data");
    fapt.add_keyring_paths(&["/usr/share/keyrings/debian-archive-keyring.gpg"])?;
    fapt.update()?;

    fapt.walk_sections(|map| {
        let pkg = map
            .get_if_one_line("Package")
            .ok_or_else(|| format_err!("invalid Package"))?;
        println!("{}", pkg);
        Ok(())
    })?;

    Ok(())
}
