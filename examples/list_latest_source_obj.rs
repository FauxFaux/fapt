use std::collections::HashMap;

use failure::err_msg;
use fapt::commands;
use fapt::system;

fn main() -> Result<(), failure::Error> {
    let mut fapt = system::System::cache_only()?;
    commands::add_sources_entries_from_str(
        &mut fapt,
        "deb-src http://deb.debian.org/debian sid main contrib",
    )
    .expect("parsing static data");
    commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    let mut package_version_files = HashMap::with_capacity(1024);

    for list in fapt.listings()? {
        for section in fapt.open_listing(&list)? {
            let pkg = section?.as_pkg()?;
            let src = pkg.as_src().ok_or(err_msg("non-source package found"))?;

            let mut urls = Vec::with_capacity(4);

            for line in &src.files {
                if line.name.ends_with(".dsc") || line.name.ends_with(".asc") {
                    continue;
                }

                urls.push(format!("{}/{}", src.directory, line.name));
            }

            assert!(package_version_files
                .entry(pkg.name.to_string())
                .or_insert_with(HashMap::new)
                .insert(pkg.version.to_string(), urls)
                .is_none());
        }
    }

    for (package, version_files) in package_version_files {
        let best = version_files
            .keys()
            .into_iter()
            .max_by(|left, right| deb_version::compare_versions(left, right))
            .unwrap();
        for file in &version_files[best] {
            println!("{} {} {}", package, best, file);
        }
    }

    Ok(())
}
