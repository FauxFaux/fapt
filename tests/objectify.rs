use std::collections::HashMap;
use std::io;

use failure::format_err;
use failure::Error;
use fapt::parse::Package;

fn parse(pkg: &str) -> Result<Package, Error> {
    Package::parse(&mut fapt::rfc822::scan(pkg).collect_to_map()?)
}

#[test]
fn parse_multiple_binaries() -> Result<(), Error> {
    let pkg = parse(include_str!("packages/alien-arena.pkg"))?;

    assert_eq!("7.66+dfsg-5", pkg.version);

    let src = pkg.as_src().unwrap();

    assert_eq!(
        vec!["alien-arena", "alien-arena-server"],
        src.binaries
            .iter()
            .map(|b| b.name.to_string())
            .collect::<Vec<String>>()
    );
    assert_eq!(HashMap::new(), pkg.unparsed);
    Ok(())
}

#[test]
fn parse_multi_line_package_list() -> Result<(), Error> {
    let pkg = parse(include_str!("packages/google-android-installers.pkg"))?;
    assert_eq!("google-android-installers", pkg.name);
    assert_eq!(31, pkg.as_src().unwrap().binaries.len());
    Ok(())
}

/// jessie (and earlier) lack the Package-List section
#[test]
fn no_package_list() -> Result<(), Error> {
    let pkg = parse(include_str!("packages/aa3d.pkg"))?;
    assert_eq!("aa3d", pkg.name);
    let bins = &pkg.as_src().unwrap().binaries;
    assert_eq!(1, bins.len());
    assert_eq!("aa3d", bins[0].name);
    Ok(())
}

#[test]
fn parse_provides() -> Result<(), Error> {
    let p = parse(include_str!("packages/python3-cffi-backend.pkg"))?;
    assert_eq!("python3-cffi-backend", p.name.as_str());
    let bin = p.as_bin().unwrap();
    assert_eq!(3, bin.provides.len());
    assert_eq!(HashMap::new(), p.unparsed);
    Ok(())
}

#[test]
fn trusty() -> Result<(), Error> {
    for section in fapt::sections_in_reader(
        io::Cursor::new(&include_bytes!("lists/trusty.list")[..]),
        "trusty.list".to_string(),
    ) {
        let section = section?;
        let p = Package::parse(&mut fapt::rfc822::scan(&section).collect_to_map()?)?;
        assert!(!p.name.is_empty());
        let bin = p.as_bin().expect("bin package");

        // TODO: this is not .. working
        assert_eq!(
            1,
            bin.file
                .as_ref()
                .ok_or_else(|| format_err!("bin has file: {:?}", p.name))?
                .size
        );
    }
    Ok(())
}
