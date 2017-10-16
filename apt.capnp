@0xaf696212bdf0eef6;

# code/apt/apt-pkg/tagfile-keys.list

struct Source {
    package     @0 :Text;
    version     @1 :Text;

    directory   @2 :Text;
    homepage    @3 :Text;
    section     @4 :Text;
    maintainer  @5 :Text;
    todoOrigMaintNotAvailable   @6 :Void;
    standards   @7 :Text;

    arch        @8 :List(Text);
    binaries    @9 :List(SourceBinary);
    buildDeps  @10 :List(Dependency);
    files      @11 :List(File);
    vcs        @12 :List(Vcs);

    todoRemove @13 :Void;

    format :union {
        unknown     @14 :Void;
        original    @15 :Void;
        quilt3dot0  @16 :Void;
        native3dot0 @17 :Void;
        git3dot0    @18 :Void;
    }

    # https://www.debian.org/doc/debian-policy/#priorities
    priority :union {
        unknown   @19 :Void;
        required  @20 :Void;
        important @21 :Void;
        standard  @22 :Void;
        optional  @23 :Void;
        extra     @24 :Void;
        source    @25 :Void;
    }

}

struct Dependency {
    package             @0 :Text;
    versionConstraints  @1 :List(Constraint);
    restrictions        @2 :List(Text);
}

struct Constraint {
    name @0 :Text;
    operator :union {
        ge @1 :Void;
        eq @2 :Void;
        le @3 :Void;
        gt @4 :Void;
        lt @5 :Void;
    }
}

struct File {
    name   @0 :Text;
    size   @1 :UInt64;
    md5    @2 :Text;
    sha1   @3 :Text;
    sha256 @4 :Text;
    sha512 @5 :Text;

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

    # TODO: struct Priority
    priority  @3 :Text;
    archSpec  @4 :Text;
}
