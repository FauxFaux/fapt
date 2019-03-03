use std::env;

use fapt::commands;
use fapt::rfc822::RfcMapExt;

fn main() -> Result<(), failure::Error> {
    let args: Vec<String> = env::args().skip(1).collect();

    let src_line = if args.is_empty() {
        "deb-src http://deb.debian.org/debian sid main contrib".to_string()
    } else {
        args.join(" ")
    };

    let mut fapt = fapt::System::cache_only()?;
    commands::add_sources_entries_from_str(&mut fapt, src_line)?;
    commands::add_builtin_keys(&mut fapt);
    fapt.update()?;

    for section in commands::all_paragraphs(&fapt)? {
        let section = section?;
        let map = section.as_map()?;
        let pkg = map.get_value("Package").one_line_req()?;
        println!("{}", pkg);
    }

    Ok(())
}
