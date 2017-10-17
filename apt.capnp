@0xaf696212bdf0eef6;

# code/apt/apt-pkg/tagfile-keys.list

struct RawSource {
    package  @0 :Text;
    version  @1 :Text;
    index    @2 :Text;

    binaries @3 :List(Text);
    files    @4 :List(File);

    entries  @5 :List(Entry);
}

struct File {
    name   @0 :Text;
    size   @1 :UInt64;
    md5    @2 :Text;
    sha1   @3 :Text;
    sha256 @4 :Text;
    sha512 @5 :Text;
}

struct Entry {
    key   @0 :Text;
    value @1 :Text;
}
