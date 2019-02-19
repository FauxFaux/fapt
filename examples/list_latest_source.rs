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

    let mut package_version_files = HashMap::with_capacity(1024);

    for list in fapt.listings()? {
        for section in fapt.open_listing(&list)? {
            let section = section?;
            let map = section.as_map()?;
            let files_section = map
                .get("Files")
                .ok_or_else(|| err_msg("no file in package"))?;
            let pkg = map.get_value("Package").one_line_req()?;
            let version = map.get_value("Version").one_line_req()?;
            let dir = map.get_value("Directory").one_line_req()?;

            let mut urls = Vec::with_capacity(4);

            for line in files_section {
                let file_name = line.split(' ').nth(2).unwrap();
                if file_name.ends_with(".dsc") || file_name.ends_with(".asc") {
                    continue;
                }

                urls.push(format!("{}/{}", dir, file_name));
            }

            assert!(package_version_files
                .entry(pkg.to_string())
                .or_insert_with(HashMap::new)
                .insert(version.to_string(), urls)
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
