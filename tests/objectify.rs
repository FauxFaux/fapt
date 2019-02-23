use failure::Error;
use fapt_pkg::Package;

fn parse(pkg: &str) -> Result<Package, Error> {
    fapt_pkg::Package::parse(&mut fapt_pkg::rfc822::scan(pkg).collect_to_map()?)
}

#[test]
fn google_android_installers() -> Result<(), Error> {
    let pkg = parse(include_str!("packages/google-android-installers.pkg"))?;
    assert_eq!("google-android-installers", pkg.name);
    assert_eq!(31, pkg.as_src().unwrap().binaries.len());
    Ok(())
}
