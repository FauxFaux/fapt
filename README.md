# Faux' apt

This is a library-like tool for interacting with Debian/Ubuntu package metadata,
like one would expect from `apt` or `aptitude`. These tools are not arranged as
libraries, however, so it is rather hard to drive them in this manner.

This project is pure, safe Rust, and runs fine on Windows, OSX, etc. It does not
need `root`, unless you want it to write to root-only directories.

It does not currently contain a way to install packages, so cannot be used as a
replacement for `apt`.

It is intended to give access to the data when necessary, for example:

```rust
let mut fapt = fapt_pkg::System::cache_dirs_only(".fapt-lists")?;
commands::add_sources_entries_from_str(&mut fapt, "deb http...")?;
commands::add_builtin_keys(&mut fapt);
fapt.update()?;

for list in fapt.listings()? {
    for package in fapt.open_listing(&list)? {
```

This can be seen in one of the examples:
```text
% cargo run -q --example \
   list_source_packages deb-src https://deb.debian.org/debian buster main non-free
Downloading: https://deb.debian.org/debian/dists/buster/InRelease ... complete.
Downloading: https://deb.debian.org/debian/dists/buster/main/source/by-hash/SHA256/c3a1781dc47ba30d2c29eafd556d36917bb180c1f55c4862fedd48da28a2042f ... complete.
0ad
0ad-data
0xffff
...
```


### Data model

Here is an example `sources list` entry on an `amd64` machine:

```text
deb     https://deb.debian.org/debian sid main contrib
deb-src https://deb.debian.org/debian sid main contrib
```

This is interpreted as:

 1. Download https://deb.debian.org/debian/dists/sid/Release. This is called the `ReleaseFile`.
 2. Look through it for entries named the following. Each of these is called a `Listing`:
    * `main/binary-amd64/Packages`
    * `contrib/binary-amd64/Packages`
    * `main/source/Sources`
    * `contrib/source/Sources`
 3. Download them all.

This result (ignoring verification and compression) is now a list of _Paragraphs_.
Each _Paragraph_ describes a package, either a source or binary package.
