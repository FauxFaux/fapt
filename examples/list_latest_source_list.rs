use fapt_pkg::PackageList;

use std::collections::HashMap;

use failure::err_msg;
use fapt_pkg::commands;
use fapt_pkg::RfcMapExt;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_only()?;
    commands::add_sources_entries_from_str(
        &mut fapt,
        "deb-src http://deb.debian.org/debian sid main contrib",
    )
    .expect("parsing static data");
    commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    let mut p = PackageList::new();

    for para in commands::all_paragraphs(&fapt)? {
        p.push(para?.as_pkg()?);
    }

    Ok(())
}
