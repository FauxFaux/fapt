use std::env;

use fapt_pkg::commands;
use fapt_pkg::RfcMapExt;

fn main() -> Result<(), failure::Error> {
    let args: Vec<String> = env::args().skip(1).collect();

    let src_line = if args.is_empty() {
        "deb-src http://deb.debian.org/debian sid main contrib".to_string()
    } else {
        args.join(" ")
    };

    let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
    commands::add_sources_entries_from_str(&mut fapt, src_line)?;
    commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    for list in fapt.listings()? {
        for section in fapt.open_listing(&list)? {
            let section = section?;
            let map = section.as_map()?;
            let pkg = map.get_value("Package").one_line_req()?;
            println!("{}", pkg);
        }
    }

    Ok(())
}
