use failure::Error;
use fapt::commands;
use fapt::Package;
use fapt::System;

fn main() -> Result<(), Error> {
    let mut fapt = System::cache_only()?;
    commands::add_builtin_keys(&mut fapt);
    commands::add_sources_entries_from_str(
        &mut fapt,
        r#"
debs http://deb.debian.org/debian       sid              main contrib non-free
debs http://deb.debian.org/debian       testing          main contrib non-free
debs http://deb.debian.org/debian       stable           main contrib non-free
debs http://deb.debian.org/debian       oldstable        main contrib non-free

debs http://archive.ubuntu.com/ubuntu/  disco            main universe multiverse restricted

debs http://archive.ubuntu.com/ubuntu/  xenial           main universe multiverse restricted
debs http://archive.ubuntu.com/ubuntu/  xenial-updates   main universe multiverse restricted

debs http://archive.ubuntu.com/ubuntu/  trusty           main universe multiverse restricted
debs http://archive.ubuntu.com/ubuntu/  trusty-updates   main universe multiverse restricted

    "#,
    )?;
    fapt.update()?;

    let mut good: u64 = 0;
    let mut done: u64 = 0;

    for listing in fapt.listings()? {
        for item in fapt.open_listing(&listing)? {
            let item = item?;
            let res = Package::parse(&mut item.as_map()?);
            if res.is_ok() {
                good += 1;
            }
            if let Err(e) = res {
                println!("{:?}", item);
                println!("{:?}", e);
            };
            done += 1;
        }
    }
    println!("{:#?}/{:#?}", good, done);
    Ok(())
}
