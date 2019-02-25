use fapt_pkg::PackageList;

use std::collections::HashMap;

use failure::err_msg;
use fapt_pkg::commands;
use fapt_pkg::RfcMapExt;

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_only()?;
    commands::add_sources_entries_from_str(
        &mut fapt,
        "deb-src http://deb.debian.org/debian sid main contrib",
    )
    .expect("parsing static data");
    commands::add_builtin_keys(&mut fapt);
//    fapt.update()?;

    let mut p = PackageList::new();

    for para in commands::all_paragraphs(&fapt)? {
        let para = para?;
        let map_view = para.as_map()?;
        let name = map_view.get("Package").unwrap();
        match name.as_slice() {
            // bad architecture: https://lintian.debian.org/tags/unknown-architecture.html
            &["reprozip"] => continue,
            // bad architecture: "[!avr]" in build-depends
            &["gcc-3.3"] => continue,
            _ => (),
        }
        p.push(para.as_pkg()?);
    }

    Ok(())
}
