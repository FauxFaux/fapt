# The Arch Packaging Model

A summary of how Arch Linux builds a binary package, written as reference for
someone who knows one packaging system and needs to understand this one. The
running example is `packages/sway/`, a real Arch package for the Sway Wayland
compositor. Authoritative sources:

- `PKGBUILD.wiki` — the annotated variable reference (mirror of the Arch wiki).
- `arch-meson.wiki` — the Meson-specific packaging guidelines.
- `packages/sway/PKGBUILD` — a complete, real build recipe.
- `packages/sway/.SRCINFO` — the machine-readable metadata dump.
- `packages/sway/sway.install` — a lifecycle-hook script.
- `man 5 PKGBUILD`, `man 8 makepkg`, `man 8 pacman`.

---

## 1. The shape of the whole thing

An Arch package is built by **one Bash script**, the `PKGBUILD`. `makepkg`
sources it as a shell script, reads a set of well-known **variables**
(`pkgname`, `depends`, `source`, …) and then calls a set of well-known
**functions** (`prepare()`, `build()`, `check()`, `package()`) in order. There
is no abstraction layer between you and the build: the PKGBUILD *is* the logic,
and everything is imperative shell.

The output is a single compressed tarball `pkgname-pkgver-pkgrel-arch.pkg.tar.zst`
containing the built files plus embedded metadata, installable with `pacman -U`.

The mental model is: **declare metadata as variables, then write shell to fetch,
build, and stage files into a fake root.** `makepkg` orchestrates; it does not
do any of the building for you.

## 2. Metadata is just shell variables

The top of `PKGBUILD` sets variables. The mandatory ones are `pkgname`,
`pkgver`, `pkgrel`, `arch`. From the sway example:

```bash
pkgname=sway
pkgver=1.12          # upstream version; NO hyphens allowed
epoch=1              # bumped only to force-supersede a broken version scheme
pkgrel=4             # Arch's own revision of this pkgver; reset to 1 on new pkgver
arch=(x86_64)
license=(MIT)
```

Version ordering is `epoch:pkgver-pkgrel`, compared with `vercmp`. `epoch`
(default 0) is the nuclear option: it trumps everything, used only when an
upstream version scheme change breaks normal comparison.

Key metadata variables (full list in `PKGBUILD.wiki`):

- `pkgdesc`, `url` — human-facing description and homepage.
- `depends` — runtime **and** build-time-implied dependencies (see §5).
- `makedepends` — needed only to build (`meson`, `ninja`, `scdoc`, …).
- `checkdepends` — needed only to run `check()`.
- `optdepends` — `'pkg: reason'` strings; not installed automatically, purely
  advisory. Sway lists 18 of them (foot, grim, swaylock…).
- `provides` / `conflicts` / `replaces` — the virtual-package and
  supersession relations.
- `backup` — config files under `/etc` that pacman must not clobber on upgrade;
  modified ones become `.pacnew`. Sway lists `etc/sway/config`.
- `options` — toggles `makepkg` behaviour (e.g. `!strip`, `!lto`).
- `install` — names a `.install` hook script (see §7).

## 3. Sources and integrity

`source=()` is an array of things to fetch or find locally. Entries can be
URLs, VCS refs, or bare filenames (resolved next to the `PKGBUILD`). Sway mixes
all styles:

```bash
source=("git+https://github.com/swaywm/sway.git#tag=$pkgver?signed"
        "50-systemd-user.conf"          # a local file, sitting beside PKGBUILD
        "sway-portals.conf"             # ditto
        "remove_git_version_format.patch")
```

Every `source` entry needs a matching entry in a checksum array
(`sha512sums`, `b2sums`, `sha256sums`, …), or `SKIP` (used for VCS/signed
sources). Integrity is the packager's responsibility, declared inline. Signed
git tags and `validpgpkeys=()` give an authenticity check on top of the hash.

`makepkg` downloads/verifies everything up front and extracts archives into
`$srcdir` before any function runs.

## 4. The build lifecycle: four functions

`makepkg` calls these in order, each with a defined job and a defined working
environment:

| function     | purpose                          | key vars available          |
|--------------|----------------------------------|-----------------------------|
| `prepare()`  | patch / pre-process sources      | `$srcdir`                   |
| `build()`    | compile                          | `$srcdir`                   |
| `check()`    | run test suite (optional)        | `$srcdir`                   |
| `package()`  | stage files into the fake root   | `$srcdir`, **`$pkgdir`**    |

Two directories drive everything:

- **`$srcdir`** — where sources are extracted and the build happens.
- **`$pkgdir`** — the **fake root**. `package()` installs into `$pkgdir` as if
  it were `/`; the tree under `$pkgdir` *becomes* the package. This is the same
  idea as `DESTDIR`.

From sway:

```bash
prepare() {
  cd "$pkgname"
  patch -Np1 -i "$srcdir/remove_git_version_format.patch"   # patching is manual
}

build() {
  mkdir -p build
  arch-meson build "$pkgname" -D sd-bus-provider=libsystemd -D werror=false -D b_ndebug=true
  ninja -C build
}

package() {
  DESTDIR="$pkgdir" ninja -C build install
  install -Dm644 "$pkgname/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 50-systemd-user.conf -t "$pkgdir/etc/sway/config.d/"
  install -Dm644 sway-portals.conf "$pkgdir/usr/share/xdg-desktop-portal/sway-portals.conf"
}
```

Note what is explicit here: **patching is a hand-written `patch` call** in
`prepare()`; **extra files are hand-copied** with `install` in `package()`.
There is no declarative "install these files" layer — you write the `cp`/`install`
commands yourself.

## 5. `arch-meson`: the thin convenience wrapper

`build()` could call `meson setup --prefix=/usr --buildtype=plain …` directly.
Instead Arch ships `arch-meson`, a "highly opinionated wrapper" (from
`arch-meson.wiki`) that pre-sets the flags an Arch package almost always wants:
`--prefix=/usr`, `--sysconfdir=/etc`, `--buildtype=plain`, LTO/pie defaults,
`-Db_lto=true`, etc. The packager then adds only software-specific `-D` options
(`-D sd-bus-provider=libsystemd`).

This is the closest Arch analogue to Debian's `dh` philosophy — factor out the
distro-wide boilerplate — but it is *much* thinner: it wraps exactly one command
(`meson setup`), not an entire build/install/package pipeline. `build`, `check`,
and `package` are still hand-written and still call `ninja`/`meson` themselves.

Meson-specific conventions (`arch-meson.wiki`): `meson compile -C build`,
`meson test -C build` in `check()`, and
`meson install -C build --destdir "$pkgdir"` in `package()`.

## 6. Dependency philosophy

`depends` is expected to list **all direct first-level dependencies**, even
transitively-satisfied ones, because pacman does not compute them for you at
build time the way Debian's `${shlibs:Depends}` does. But Arch has a partial
automation: if a `depends` entry looks like a shared library
(`libjson-c.so`, `libwayland-server.so`), `makepkg` inspects the built binaries
and pins the exact soname version needed. Sway's `depends` mixes plain package
names (`cairo`, `pango`) with soname deps (`libinput`… `libudev.so`).

There is no separate `-dev` / `-doc` / runtime split by convention: an Arch
package is usually **one package with everything in it** (headers, binaries,
docs together). Splitting exists (`pkgbase` + a `pkgname` array) but is the
exception, not the rule.

## 7. Install hooks (`*.install`)

The `install=sway.install` variable points to a script with well-known
functions that pacman runs at lifecycle points *on the user's machine*:
`pre_install`, `post_install`, `pre_upgrade`, `post_upgrade`, `pre_remove`,
`post_remove`. Sway's uses `post_install`/`post_upgrade` to print a setup notice
and to `setcap cap_sys_nice+ep usr/bin/sway`. This is the analogue of Debian's
maintainer scripts (`postinst`, `prerm`, …) — but a single file with functions
rather than separate scripts.

## 8. `.SRCINFO`

`.SRCINFO` is a flat, machine-readable dump of all the metadata that would
otherwise require *executing* the PKGBUILD to discover (since it is arbitrary
shell). It is generated with `makepkg --printsrcinfo` and committed alongside
the PKGBUILD, primarily so the AUR and tooling can read metadata without
sourcing untrusted Bash. It carries no logic — it is derived, not authored.

## 9. What to take away when mapping to Debian

- Arch metadata is **imperative shell variables**; Debian's is **declarative
  RFC822 control files**.
- Arch gives you **four lifecycle functions you fill in**; Debian gives you a
  **sequence of ~50 helper tools that run automatically** (`dh`), which you only
  *override* where needed.
- Arch: one fat package by default. Debian: split into binary packages
  (`sway`, `sway-backgrounds`, and conventionally `-dev`/`-doc`) from one source.
- Arch fake-root = `$pkgdir`; Debian's is `debian/tmp` + `debian/<pkg>/`.
- Arch patching/extra-file-installs are **hand-written shell**; Debian has
  **declarative mechanisms** (`debian/patches` + quilt, `debian/*.install`).
- The overriding of `--prefix=/usr`, reduced deps, extra patches, and custom
  install paths that this project cares about are all done in Arch by *editing
  the shell directly*. In Debian the same goals are met by *configuring the
  declarative machinery* — which is the subject of `../deb/DEBIAN.md`.
