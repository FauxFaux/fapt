#!/usr/bin/env python3
# fields apt likes, and we like too; we're going to parse them
import os
import re

from typing import Iterable, Tuple

HANDLED_FIELDS_SOURCE = {
    # core
    'Package',
    'Source',
    'Version',

    # mapped into proper types
    'Priority',
    'Architecture',
    'Format',

    # parsed into Binaries
    'Binary',
    'Package-List',

    # parsed into Files
    'Files',

    # typo of Original-Maintainer, upstart in xenial
    'Orig-Maintainer',

    # parsed build-deps
    'Build-Conflicts',
    'Build-Conflicts-Arch',
    'Build-Conflicts-Indep',
    'Build-Depends',
    'Build-Depends-Arch',
    'Build-Depends-Indep',

    # folded into Files
    'Checksums-Md5',
    'Checksums-Sha1',
    'Checksums-Sha256',
    'Checksums-Sha512',

    # parsed into Identities
    'Maintainer',
    'Original-Maintainer',
    'Uploaders',
}

# What a mess.
for vcs in [
    'Arch',
    'Browse',
    'Browser',
    'Bzr',
    'Cvs',
    'Darcs',
    'Git',
    'Hg',
    'Mtn',
    'Svn',
]:
    HANDLED_FIELDS_SOURCE.add('Vcs-' + vcs)
    HANDLED_FIELDS_SOURCE.add('Orig-Vcs-' + vcs)
    HANDLED_FIELDS_SOURCE.add('Original-Vcs-' + vcs)
    HANDLED_FIELDS_SOURCE.add('Debian-Vcs-' + vcs)
    HANDLED_FIELDS_SOURCE.add('Upstream-Vcs-' + vcs)
    HANDLED_FIELDS_SOURCE.add('Vcs-Upstream-' + vcs)

# TODO: this alias needs to be at handled time, not here
ALIASES_SOURCE = {
    # 'Orig-Maintainer': 'Original-Maintainer'
}

HANDLED_FIELDS_SOURCE.update(ALIASES_SOURCE.keys())

# finding new fields:
# ../raw/build/apt-dump raw-sources | cargo run --release | capnp decode ../apt.capnp Source --short | sed -n 's/.*unparsed = (//p' | sed 's/", /"\n/g' | cut -d= -f 1 | sort | uniq -c | sort -n

KNOWN_FIELDS_SOURCE = [
    # definitely just normal strings, don't need parsing
    'Directory',
    'Homepage',
    'Standards-Version',
    'Section',

    # should enum up Testsuite, and parse package list out of Triggers
    # https://anonscm.debian.org/git/lintian/lintian.git/tree/checks/testsuite.pm
    'Testsuite',
    'Testsuite-Triggers',
    'Testsuite-Restrictions',

    # booleans?
    'Autobuild',
    'Dm-Upload-Allowed',

    # Fields that have been seen in the wild, but which apt ignores.
    'Extra-Source-Only',

    'Build-Indep-Architecture',

    'Dgit',

    'Go-Import-Path',
    'Python-Version',
    'Python3-Version',
    'Ruby-Versions',

    'Comment',

    # apt fields
    'Origin',
]

HANDLED_FIELDS_BINARY = {
    'Package',
    'Version',

    # folded into File
    'Filename',
    'Size',
    'MD5sum',
    'SHA1',
    'SHA256',
    'SHA512',

    # not text
    'Installed-Size',
    'Essential',
    'Build-Essential',
    'Priority',
    'Architecture',

    # package lists
    'Depends',
    'Recommends',
    'Suggests',
    'Enhances',
    'Pre-Depends',
    'Breaks',
    'Conflicts',
    'Replaces',
    'Provides',

    # folded into Description
    'Description',
    'Description-md5',

    # parsed as Identities
    'Maintainer',
    'Original-Maintainer',
}

KNOWN_FIELDS_BIN = [
    'Homepage',
    'Section',
    'Source',
    'Task',

    'Bugs',
    'Supported',
    'Origin',

    'Status',

    # should probably convert to types
    'Build-Ids',  # List[Sha1]
    'Multi-Arch',  # {'same', 'foreign'}
    'Package-Type',  # {'ddeb'}
    'Auto-Built-Package',  # {'debug-symbols'}
    'Built-Using',  # List[Dependency]

    # junk seen in the wild
    'Modaliases',

    'Gstreamer-Decoders',
    'Gstreamer-Elements',
    'Gstreamer-Encoders',
    'Gstreamer-Uri-Sinks',
    'Gstreamer-Uri-Sources',
    'Gstreamer-Version',

    'License',
    'Vendor',

    'Go-Import-Path',
    'Python-Version',
    'Python3-Version',
    'Ruby-Versions',
    'Lua-Versions',

    'Python-Egg-Name',
    'Ghc-Package',

    'Npp-Applications',
    'Npp-Description',
    'Npp-File',
    'Npp-Mimetype',
    'Npp-Name',

    'Postgresql-Catversion',
    'Postgresql-Version',
    'Tads2-Version',
    'Tads3-Version',
    'Xul-Appid',

    'Phased-Update-Percentage',

    # original apt list
    'Built-For-Profiles',
    'Class',
    'Conffiles',
    'Config-Version',
    'Files',
    'Important',
    'Installer-Menu-Item',
    'Kernel-Version',
    'MSDOS-Filename',
    'Optional',
    'Package-Revision',
    'Recommended',
    'Revision',
    'Subarchitecture',
    'Tag',
    'Triggers-Awaited',
    'Triggers-Pending',

    # TODO: should be an alias for Original-Maintainer; typo
    'Orig-Maintainer',
]

ALIASES_BINARY = {
    'Package_Revision': 'Package-Revision',
}


def to_snake(s: str) -> str:
    return re.sub(r'(?!^)[_-]([a-zA-Z])', lambda m: m.group(1).upper(), s.lower())


def to_rust(s: str) -> str:
    return re.sub(r'[_-]', '_', s.lower())


def main():
    fields_source = []
    fields_binary = []
    for field in KNOWN_FIELDS_SOURCE:
        if field not in HANDLED_FIELDS_SOURCE:
            fields_source.append(field)

    capnp_format_string_source, rust_format_string_source = make_format_strings(fields_source)

    for field in KNOWN_FIELDS_BIN:
        if field not in HANDLED_FIELDS_BINARY:
            fields_binary.append(field)

    capnp_format_string_bin, rust_format_string_binary = make_format_strings(fields_binary)

    with open('../apt.capnp~', 'w') as tmp:
        with open('../apt.capnp') as orig:
            for line in orig:
                tmp.write(line)
                if '## generated by gen.py' == line.strip():
                    break

        tmp.write("""
struct UnparsedSource {
""")
        for i, field in enumerate(fields_source):
            tmp.write(capnp_format_string_source.format(to_snake(field), i))

        tmp.write("""}

struct UnparsedBinary {
""")

        for i, field in enumerate(fields_binary):
            tmp.write(capnp_format_string_bin.format(to_snake(field), i))

        tmp.write("}\n")

    os.rename('../apt.capnp~', '../apt.capnp')

    with open('src/fields.rs', 'w') as rs:
        rs.write("""// GENERATED by gen.py; do not edit
#![cfg_attr(rustfmt, rustfmt_skip)]

use apt_capnp::unparsed_source;
use apt_capnp::unparsed_binary;
use errors::*;
use blank_to_null;
""")

        gen_rust(rs, 'source', fields_source, rust_format_string_source, HANDLED_FIELDS_SOURCE, ALIASES_SOURCE)
        gen_rust(rs, 'binary', fields_binary, rust_format_string_binary, HANDLED_FIELDS_BINARY, ALIASES_BINARY)


def gen_rust(rs, tag: str, fields_source: Iterable[str], format_string: str, handled_fields, aliases):
    rs.write("""

pub fn set_field_{0}(key: &str, val: &str, builder: &mut unparsed_{0}::Builder) -> Result<bool> {{
    match key {{
""".format(tag))

    for orig in sorted(fields_source):
        rs.write(format_string.format(orig, to_rust(orig)))

    rs.write("\n        // Typos\n")
    for key, val in aliases.items():
        rs.write(format_string.format(key, to_rust(val)))

    rs.write("""
        _ => return Ok(false),
    }}

    Ok(true)
}}
""".format(tag))


def make_format_strings(fields: Iterable[str]) -> Tuple[str, str]:
    max_len = max(len(to_snake(field)) for field in fields)
    capnp_format_string = ('    {: <' + str(max_len) + '} @{} :Text;\n')
    rust_format_string = '        "{}" => blank_to_null(val, |x| builder.set_{}(x)),\n'
    return capnp_format_string, rust_format_string


if __name__ == '__main__':
    main()
