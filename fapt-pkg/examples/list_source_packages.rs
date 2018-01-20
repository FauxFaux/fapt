#[macro_use]
extern crate error_chain;

extern crate fapt_pkg;

use errors::*;

fn run() -> Result<()> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    fapt.add_sources_entry_line("deb-src http://deb.debian.org/debian sid main contrib")
        .expect("parsing static data");
    fapt.add_keyring_paths(&["/usr/share/keyrings/debian-archive-keyring.gpg"])?;
    fapt.update()?;

    fapt.walk_sections(|map| {
        let pkg = map.get_if_one_line("Package").ok_or("invalid Package")?;
        println!("{}", pkg);
        Ok(())
    })?;

    Ok(())
}

quick_main!(run);

mod errors {
    error_chain!{
        links {
            FaptPkg(::fapt_pkg::Error, ::fapt_pkg::ErrorKind);
        }
    }
}
