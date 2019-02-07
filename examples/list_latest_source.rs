use std::collections::HashMap;

use failure::err_msg;

fn main() -> Result<(), failure::Error> {
    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    fapt.add_sources_entry_line("deb-src http://deb.debian.org/debian sid main contrib")
        .expect("parsing static data");
    fapt.add_keyring_paths(&["/usr/share/keyrings/debian-archive-keyring.gpg"])?;
    fapt.update()?;

    let mut package_version_files = HashMap::with_capacity(1024);

    fapt.walk_sections(|map| {
        let files_section = map
            .get("Files")
            .ok_or_else(|| err_msg("no file in package"))?;
        let pkg = map
            .get_if_one_line("Package")
            .ok_or_else(|| err_msg("invalid Package"))?;
        let version = map
            .get_if_one_line("Version")
            .ok_or_else(|| err_msg("invalid Version"))?;
        let dir = map
            .get_if_one_line("Directory")
            .ok_or_else(|| err_msg("invalid Directory"))?;

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
        Ok(())
    })?;

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
