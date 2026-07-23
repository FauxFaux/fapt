# The Debian Packaging Model

A summary of how a modern Debian source package builds one or more binary
`.deb`s, written as a companion to `../arch/ARCH.md`. The running example is
`sway-1.11/`, the real Debian packaging for the Sway Wayland compositor (source
version `1.11-3`). Authoritative sources:

- `man 7 debhelper` — the tool-suite overview.
- `man 1 dh` — the command **sequencer** and override/hook mechanism.
- `man 1 dh_auto_configure`, `dh_install`, `dh_missing`, … — individual helpers.
- `sway-1.11/debian/` — a complete, real `debian/` directory.
- Debian Policy (`Standards-Version: 4.7.0` in `debian/control`).

Everything below is grounded in what `dh` on this machine actually reports it
would run (`dh binary --no-act`), not just doctrine.

---

## 1. The shape of the whole thing

A Debian source package is **not a script**. It is the upstream source tree with
a `debian/` subdirectory bolted on. `debian/` contains a set of **declarative
control files** plus a `debian/rules` file that is *technically* a Makefile but
in modern practice is almost empty — it delegates to `dh`, the debhelper
sequencer.

`dpkg-buildpackage` drives the top-level build; it invokes `debian/rules` with
standardized targets (`clean`, `build`, `binary`); `debian/rules` calls `dh`;
`dh` runs a long **sequence of `dh_*` helper tools**, each doing one small,
policy-defined job. The output is one or more `.deb`s (here: `sway` and
`sway-backgrounds`) plus the source package (`.dsc` + `.orig.tar.gz` +
`.debian.tar.xz`, all visible in `deb/`).

The mental model is the inverse of Arch: **you do not write the build steps; you
declare metadata and let a fixed pipeline run, overriding individual steps only
where the default is wrong.**

## 2. `debian/rules` — a Makefile that mostly gets out of the way

The entire real build logic for sway is this (`debian/rules`):

```makefile
#!/usr/bin/make -f
export DH_VERBOSE=1
export DH_OPTIONS=-v

include /usr/share/dpkg/pkg-info.mk
export DEB_BUILD_MAINT_OPTIONS = hardening=+all
export DEB_CFLAGS_MAINT_APPEND = -Wno-error=deprecated-declarations

%:
	dh $@

override_dh_auto_configure:
	dh_auto_configure -- --libexecdir=lib

override_dh_missing:
	dh_missing --fail-missing

override_dh_install:
	if ! dh_install; then tree debian/tmp; exit 1; fi
```

The `%: / dh $@` catch-all rule is the key line: **every rules target is
answered by "run `dh` for that sequence."** `dh build` runs the build sequence,
`dh binary` runs the full binary sequence, etc. Everything else in this file is
either environment tuning (hardening flags, a compiler-flag append) or an
**override target** (§4).

`DEB_BUILD_MAINT_OPTIONS = hardening=+all` turns on the full set of compiler
hardening flags distro-wide — the Debian analogue of what `arch-meson` bakes in.

## 3. The `dh` sequence: ~50 tools you didn't have to write

`dh binary` expands into a fixed, ordered pipeline. Abbreviated from
`dh binary --no-act` on this machine, the load-bearing steps are:

```
build seq:    dh_auto_configure → dh_auto_build → dh_auto_test
install seq:  dh_auto_install → dh_install → dh_installdocs → dh_installman
              → dh_installchangelogs → dh_installsystemd(user) → …
binary seq:   … → dh_link → dh_strip → dh_makeshlibs → dh_shlibdeps
              → dh_gencontrol → dh_md5sums → dh_builddeb
```

Each `dh_*` tool is small and single-purpose (`man 7 debhelper` lists them all):

- **`dh_auto_configure`** — detects the build system (here Meson, from
  `meson.build`) and runs it with Debian's standard flags: `--prefix=/usr`,
  `--sysconfdir=/etc`, `--buildtype=plain`, the hardening `CFLAGS`, multiarch
  libdir, etc. This is the direct counterpart to `arch-meson`, but it is
  *automatic* — you don't name the build system, debhelper sniffs it.
- **`dh_auto_build` / `dh_auto_test`** — `ninja` / `meson test`.
- **`dh_auto_install`** — `meson install` into the staging root `debian/tmp`
  (the `$pkgdir`/`DESTDIR` analogue).
- **`dh_install`** — distributes files *out of* `debian/tmp` *into* per-package
  staging dirs `debian/sway/`, `debian/sway-backgrounds/`, driven by the
  `*.install` files (§6).
- **`dh_strip`, `dh_makeshlibs`, `dh_shlibdeps`** — split debug symbols and,
  crucially, **compute runtime library dependencies automatically** by
  inspecting ELF `NEEDED` entries. This is why `debian/control` can say just
  `${shlibs:Depends}` instead of listing sonames by hand.
- **`dh_gencontrol` → `dh_builddeb`** — generate the final control metadata and
  assemble the `.deb`.

You get all of this for free by writing `dh $@`. Compare Arch, where `build()`
and `package()` spell out `ninja`, `ninja install`, and every `install -Dm644`
by hand.

## 4. Overriding and hooking steps

When a default step is wrong, you **override just that step** without touching
the rest of the pipeline. Three mechanisms (`man 1 dh`):

- `override_dh_foo:` — replace what `dh_foo` does. Sway overrides
  `dh_auto_configure` to append `--libexecdir=lib` to the Meson invocation:
  ```makefile
  override_dh_auto_configure:
      dh_auto_configure -- --libexecdir=lib
  ```
  (Everything after `--` is passed straight to the underlying build system —
  this is the exact seam where **custom build options / install-path overrides**
  go.)
- `execute_before_dh_foo:` / `execute_after_dh_foo:` — inject commands around a
  step while keeping the step itself.
- An **empty** override target skips a step entirely (e.g. to disable stripping,
  `override_dh_strip:` with nothing under it).

Sway also overrides `dh_missing --fail-missing` (turn "files built but not
installed into any package" into a hard error — a completeness check with no
Arch equivalent) and wraps `dh_install` to dump the staging tree on failure.

## 5. `debian/control` — declarative metadata for a *set* of packages

`control` is an RFC822-style file with one **source stanza** and one **binary
stanza per output `.deb`**. This is where Debian's "one source → many binaries"
model lives. Sway's `control` declares:

- **Source stanza**: `Source: sway`, the `Maintainer`/`Uploaders`,
  `Standards-Version` (policy compliance level), `Vcs-*` links, and crucially
  **`Build-Depends:`** — the `makedepends` analogue, listing `-dev` packages
  (`libwlroots-0.19-dev (>= 0.19.0)`, `meson (>=0.60)`, `scdoc`, …) with version
  constraints and even alternatives (`libsystemd-dev | libelogind-dev`).
- **Binary stanza `Package: sway`**: `Architecture: any`, and dependency fields
  that are the heart of Debian's dependency model:
  - `Depends: … ${misc:Depends}, ${shlibs:Depends}` — the `${…}` are
    **substitution variables** filled in automatically by `dh_shlibdeps` /
    `dh_gencontrol`. You rarely list libraries by hand.
  - `Recommends:` (polkitd, foot, wmenu) — installed by default but removable.
  - `Suggests:` (swayidle, swaylock, xdg-desktop-portal-*) — the closest match
    to Arch's `optdepends`.
- **Binary stanza `Package: sway-backgrounds`**: `Architecture: all` (arch-
  independent data), `Multi-Arch: foreign`. This is the split-out data package —
  in Arch this would just be more `install` lines in one `package()`.

Debian's dependency **strengths** (`Depends` > `Recommends` > `Suggests`) are a
richer vocabulary than Arch's `depends` + advisory `optdepends`.

## 6. `*.install` — declarative file placement

Instead of hand-written `install -Dm644` calls, each binary package gets a
`debian/<pkg>.install` file: a list of source paths (relative to `debian/tmp`,
or to the source tree) and optional destination dirs. `dh_install` reads it and
copies. From `debian/sway.install`:

```
debian/config.d/50-systemd-user.conf /etc/sway/config.d/
debian/sway-portals.conf /usr/share/xdg-desktop-portal/
etc/sway
usr/bin/sway*
usr/share/man/man1/sway*
usr/share/zsh/vendor-completions/_sway*
…
```

Compare sway's Arch `package()`, which does the same placement with imperative
`install` commands. `debian/sway-backgrounds.install` similarly claims
`usr/share/backgrounds`, which is how the same build gets **partitioned into two
`.deb`s**. `dh_missing --fail-missing` (from the rules override) then guarantees
every built file was claimed by *some* package.

## 7. `debian/patches` — quilt, not hand-rolled `patch`

`debian/source/format` here contains `3.0 (quilt)`. That means upstream source
is kept pristine (in the `.orig.tar.gz`) and all Debian changes live as a
**quilt patch series** in `debian/patches/`, applied automatically at build
time. `debian/patches/series` lists them in order:

```
0001-Install-zsh-completions-into-the-correct-directory.patch
0002-Modify-default-configuration-for-Debian.patch
0004-feat-comment-default-output-setting.patch
0005-xdg-shell-fix-reported-WM-capabilities.patch
0006-revert_drag_lock_default_to_disabled.patch
```

These are `git format-patch`-style patches with proper headers, managed with the
`quilt` tool (`quilt new`, `quilt add`, `quilt refresh`). Contrast Arch, where
sway's single patch is applied by a literal `patch -Np1` call the packager wrote
in `prepare()`. The `.pc/` directory is quilt's private state (which patches are
currently applied) and is not authored by hand.

This is the natural home for the **"further patches"** requirement of this
project: drop a new file in `debian/patches/`, add it to `series`.

## 8. The other declarative control files

`debian/` collects many small files, each read by a specific `dh_*` tool:

- `changelog` — **the source of the package version.** The top entry
  (`sway (1.11-3) unstable; …`) defines version `1.11-3` and the target suite.
  `dh_gencontrol` and `dpkg` parse it; `dh_installchangelogs` ships it. Every
  upload appends a stanza. There is no separate `pkgrel` variable — the
  `-3` Debian revision lives *in the changelog*.
- `copyright` — machine-readable (DEP-5) per-file licensing. Debian is far
  stricter here than Arch's single `license=(MIT)`; every distinct license and
  copyright holder in the tree is enumerated.
- `*.maintscript` — declarative shortcuts for common maintainer-script actions.
  Sway's `sway.maintscript` has `rm_conffile …` to retire an old conffile on
  upgrade. For arbitrary logic you instead ship `postinst`/`prerm`/`postrm`
  scripts (the analogue of Arch's `.install` functions; the `.install` files in
  `debian/` here are *file lists*, unrelated to Arch's `.install` hook scripts —
  a naming false-friend to watch for).
- `watch` — how `uscan` finds new upstream releases (here, GitHub tags).
- `upstream/metadata`, `upstream/signing-key.asc` — upstream provenance.
- `gbp.conf` — configures `git-buildpackage` (branch layout, pristine-tar).
- `source/format` — `3.0 (quilt)`, discussed above.
- `config.d/` — actual Debian-shipped config snippets referenced by the
  `.install` files.

## 9. Conffiles and `/etc`

Files installed under `/etc` are automatically registered as **conffiles** by
`dh_installdeb`; dpkg then preserves user modifications across upgrades and
prompts on conflict. This is automatic from install location, unlike Arch which
requires an explicit `backup=()` array naming each path.

## 10. What to take away when mapping from Arch

| concern                 | Arch (`PKGBUILD`)                     | Debian (`debian/`)                                  |
|-------------------------|---------------------------------------|-----------------------------------------------------|
| overall model           | imperative Bash script                | declarative files + auto pipeline (`dh`)            |
| build steps             | you write `build()`/`package()`       | `dh_auto_*` run automatically; override as needed   |
| distro build flags      | `arch-meson` wrapper                  | `dh_auto_configure` + `DEB_BUILD_MAINT_OPTIONS`     |
| custom configure opts   | edit `build()`                        | `override_dh_auto_configure: … -- <opts>`           |
| runtime deps            | hand-listed `depends` (+soname magic) | `${shlibs:Depends}` computed by `dh_shlibdeps`      |
| build deps              | `makedepends`                         | `Build-Depends:` in `control`                       |
| optional deps           | `optdepends` (advisory)               | `Recommends:` / `Suggests:`                         |
| one source → N packages | rare split packages                   | normal: one binary stanza + `*.install` per `.deb`  |
| file placement          | `install -Dm644` in `package()`       | declarative `debian/<pkg>.install`                  |
| patching                | manual `patch -Np1` in `prepare()`    | quilt series in `debian/patches/` + `series`        |
| version / revision      | `pkgver` + `pkgrel` vars              | top of `debian/changelog` (`1.11-3`)                |
| lifecycle hooks         | `*.install` functions                 | `postinst`/`prerm`/… + `*.maintscript`              |
| `/etc` preservation     | explicit `backup=()`                  | automatic conffile detection                        |
| licensing               | `license=()` identifier              | full DEP-5 `debian/copyright`                       |
| staging root            | `$pkgdir`                             | `debian/tmp` then `debian/<pkg>/`                   |

For this project's goals — **reduced dependencies, further patches, overridden
install paths** — the Debian levers are, respectively: edit `Build-Depends`/
`Depends` and pass feature-disabling flags via `override_dh_auto_configure`; add
files under `debian/patches/` + `series`; and pass `--libexecdir=`/`--prefix=`-
style options after the `--` in `override_dh_auto_configure` (plus adjust the
`*.install` file lists). All without writing a build script.
