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

struct Source {
    package     @0 :Text;
    version     @1 :Text;

    directory   @2 :Text;
    homepage    @3 :Text;
    section     @4 :Text;
    maintainer  @5 :Text;
    origMaint   @6 :Text;
    priority    @7 :Priority;
    standards   @8 :Text;

    arch        @9 :List(Text);
    binaries   @10 :List(SourceBinary);
    files      @11 :List(File);
    vcs        @12 :List(Vcs);

    buildDep           @13 :List(Dependency);
    buildDepArch       @14 :List(Dependency);
    buildDepIndep      @15 :List(Dependency);
    buildConflict      @16 :List(Dependency);
    buildConflictArch  @17 :List(Dependency);
    buildConflictIndep @18 :List(Dependency);

    format :union {
        unknown     @19 :Void;
        original    @20 :Void;
        quilt3dot0  @21 :Void;
        native3dot0 @22 :Void;
        git3dot0    @23 :Void;
    }

    uploaders         @24 :Text;
    index             @25 :Text;
}

struct Dependency {
    alternate @0 :List(SingleDependency);
}

struct SingleDependency {
    package            @0 :Text;
    arch               @1 :Text;
    versionConstraints @2 :List(Constraint);
    archFilter         @3 :List(Text);
    stageFilter        @4 :List(Text);
}

struct Constraint {
    version @0 :Text;
    operator :union {
        ge @1 :Void;
        eq @2 :Void;
        le @3 :Void;
        gt @4 :Void;
        lt @5 :Void;
    }
}

struct Vcs {
    description @0 :Text;
    type :union {
        browser @1 :Void;
        arch    @2 :Void;
        bzr     @3 :Void;
        cvs     @4 :Void;
        darcs   @5 :Void;
        git     @6 :Void;
        hg      @7 :Void;
        mtn     @8 :Void;
        svn     @9 :Void;
    }
}

struct SourceBinary {
    name      @0 :Text;
    style     @1 :Text;
    section   @2 :Text;

    priority  @3 :Priority;
    extras    @4 :List(Text);
}

# https://www.debian.org/doc/debian-policy/#priorities
struct Priority {
    union {
        unknown   @0 :Void;
        required  @1 :Void;
        important @2 :Void;
        standard  @3 :Void;
        optional  @4 :Void;
        extra     @5 :Void;
        source    @6 :Void;
    }
}
