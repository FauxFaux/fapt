use std::fs;

use failure::err_msg;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    fapt.add_sources_entry_line("deb-src http://deb.debian.org/debian sid main contrib")
        .expect("parsing static data");
    fapt.add_keys_from(fs::File::open(
        "/usr/share/keyrings/debian-archive-keyring.gpg",
    )?)?;
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
