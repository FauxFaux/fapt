# Faux' apt

[![Build status](https://ci.appveyor.com/api/projects/status/daao4tjdcnojue5m/branch/master?svg=true)](https://ci.appveyor.com/project/FauxFaux/fapt/branch/master)
[![](https://img.shields.io/crates/v/fapt.svg)](https://crates.io/crates/fapt)

This is a library-like tool for interacting with Debian/Ubuntu package metadata,
like one would expect from `apt` or `aptitude`. These tools are not arranged as
libraries, however, so it is rather hard to drive them in this manner.

This project is pure, safe Rust, and runs fine on Windows, OSX, etc. It does not
need `root`, unless you want it to write to root-only directories.

It does not currently contain a way to install packages, so cannot be used as a
replacement for `apt`.

It is intended to give access to the data when necessary, for example:

```rust
let mut fapt = System::cache_only()?;
commands::add_sources_entries_from_str(&mut fapt, src_line)?;
commands::add_builtin_keys(&mut fapt);
fapt.update()?;

for block in commands::all_blocks(&fapt)? {
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

There is also support for parsing these `HashMap<String, Strings>`s into a proper object.

This difference between the two APIs can be seen in the `list_latest_source_map` (for the map API),
and the `list_latest_source_obj` (for the object API). The `map` API is more stable, as it does
less work, and leaves parsing and error handling to you.


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

This result (ignoring verification and compression) is now a list of _Blocks_.

Each _Block_ contains multiple _Fields_.

Each group of _Fields_ probably describes a _Package_,
either a _source_ or _binary_ package.
