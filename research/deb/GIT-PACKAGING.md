# Packaging with Git — the `gbp` (git-buildpackage) Workflow

Where `DEBIAN.md` explains the *contents* of `debian/` and `QUILT.md` explains
the patch tool, this document explains how a maintainer **manages the whole
package in a git repository** and drives builds from it. This is the layer the
sway packaging actually lives in — note `debian/gbp.conf` and its
`debian-branch = debian/sid`, `pristine-tar = True`.

Sources:
- `packaging-with-git.wiki` — the Debian wiki "PackagingWithGit" guide.
- `man 1 gbp-buildpackage` — the build driver.
- `man 1 gbp-pq` — git-native patch-queue management (a quilt alternative).
- `sway-1.11/debian/gbp.conf`, `debian/changelog`, `debian/watch`.

---

## 1. The mental model: branches, not tarballs

`git-buildpackage` (command prefix `gbp`) organises a package as a set of git
branches, following [DEP-14](https://dep-team.pages.debian.net/deps/dep14/)
naming:

- **`upstream/latest`** (or `upstream/<version>` tags) — the pristine upstream
  source, exactly as released. No `debian/` dir.
- **`debian/latest`** (sway uses **`debian/sid`**) — the upstream source *plus*
  the `debian/` directory. This is where packaging work happens.
- **`pristine-tar`** — an orphan branch storing binary deltas that let gbp
  regenerate the *bit-identical* upstream `.orig.tar.gz` from git (see §5).
- **`patch-queue/<branch>`** — a transient branch used by `gbp pq` (§6).

Configuration lives in `gbp.conf` files, read in order: `/etc/git-buildpackage/`,
`~/.gbp.conf` (personal defaults), then the project's `debian/gbp.conf` or
`.git/gbp.conf`. Options are written **without** the `git-` prefix that the
command line uses:

```ini
# sway-1.11/debian/gbp.conf
[DEFAULT]
pristine-tar = True
debian-branch = debian/sid
```

So the CLI `--git-debian-branch=debian/sid` and the conf `debian-branch =
debian/sid` are the same knob.

## 2. The four lifecycle stages

The wiki frames packaging as four stages. This is the useful high-level map:

| stage       | what happens                                                   | key tools                          |
|-------------|----------------------------------------------------------------|------------------------------------|
| **Import**  | get upstream source into git; describe how to get updates      | `gbp import-orig`, `debian/watch`  |
| **Patch**   | create/modify `debian/`, patch source as needed                | `dh_make`/`debmake`, quilt/`gbp pq`, `dch` |
| **Build**   | produce & test `.deb`s in a clean chroot                       | `gbp buildpackage`, lintian/piuparts/autopkgtest |
| **Publish** | tag the release, push, upload (or request sponsorship)         | `gbp buildpackage --git-tag`       |

## 3. Import stage

Getting upstream code into the repo. Three cases from the wiki:

- **From tarballs** — the classic path:
  ```sh
  git init --initial-branch="$(gbp config DEFAULT.debian-branch)" pkg.git
  cd pkg.git
  gbp import-orig https://…/some-project_1.0.tar.gz
  ```
  This populates the upstream and debian branches, creates an `upstream/1.0`
  tag, and (if `pristine-tar=True`) records the tarball on the `pristine-tar`
  branch. `gbp import-orig` **discards any top-level `debian/`** from the
  tarball.
- **From an upstream git repo** — clone it, rename the branch to
  `debian/latest`, and set `upstream-tag=v%(version)s` in `debian/gbp.conf` so
  gbp knows the tag pattern (sway upstream tags are `1.12`, imported via the
  `debian/watch` git mode). With a pure-git upstream you typically set
  `pristine-tar = False` — there's no tarball to track.
- **Both** — import the tarball, add the git repo as a remote, and
  `git diff upstream/1.0 v1.0` to confirm they match.

Once `debian/watch` and `debian/changelog` exist, `gbp import-orig --uscan`
fetches and imports whatever new upstream version `uscan` finds. sway's
`debian/watch` uses `mode=git, pgpmode=gittag` against the GitHub tag list.

## 4. Patch stage

Creating the `debian/` directory (once, via `dh_make` or `debmake` on an
extracted tarball, then moving the result into the repo) and thereafter:

- **`debian/changelog` is the version + target-suite control point.** Edit with
  `dch` (`dch --edit`, `dch --new-version`, `dch --release`). The top stanza's
  version (`sway (1.11-3) unstable; …`) sets *both* the package version and the
  destination suite. `dch` enforces the fiddly format. `gbp dch` can *generate*
  changelog entries from git commit messages (`gbp dch --snapshot` for WIP
  builds, later collapsed on release). `UNRELEASED` in the distribution field
  marks work-in-progress; changing it to `unstable`/`experimental` is part of
  releasing.
- **`debian/copyright`** — DEP-5 machine-readable licensing; assisted by
  `licensecheck` / `cme`.
- **Patching source** — changes *outside* `debian/` become quilt patches
  (`QUILT.md`) or `gbp pq` commits (§6). Debian requires e.g. man pages, so
  missing-upstream files may need adding here.

## 5. Build stage — `gbp buildpackage`

`gbp buildpackage` builds source + binary packages straight from the repo. In
order (from `man gbp-buildpackage`), it will:

1. Verify it's run from the right place, the tree has **no uncommitted
   changes**, and you're on the configured `debian-branch` (safety checks — so
   you don't release from a topic branch or with dirty state).
2. Optionally run a `--git-cleaner`.
3. Optionally export the tree to a separate build area (`--git-export-dir`).
4. **Build the `.orig` tarball if missing** — from `pristine-tar`
   (`--git-pristine-tar`) or `git archive` of the upstream tag/tree.
5. Run pre-build hook, then invoke the **builder**.
6. Optionally `--git-tag` the commit, then post-build / post-tag hooks.

### The builder and clean-chroot builds

By default the builder is `debuild -i -I`, which runs `debian/rules` (→ `dh`,
per `DEBIAN.md`) on the *host*. But packages should build in a **clean chroot**
so host contamination can't create "works on my machine" dependencies:

```sh
gbp buildpackage --git-pbuilder --git-dist=sid --git-arch=amd64
```

`--git-pbuilder` switches the builder to `git-pbuilder` (→ cowbuilder/pbuilder),
building inside a minimal chroot for the named distribution and architecture.
`--git-qemubuilder` does the same for foreign architectures via qemu.
`--git-dist=DEP-14` derives the distribution from the branch name.

### Passing options through

Anything on the command line **not** starting with `--git-` is passed to the
builder (and thence to `dpkg-buildpackage`). So `gbp buildpackage -S -d` builds
a source-only package; `-nc` skips clean; etc.

### Hooks

`--git-preexport`, `--git-postexport`, `--git-prebuild`, `--git-postbuild`,
`--git-posttag` inject commands at each stage (each with `GBP_*` env vars). The
canonical use is `--git-postbuild` to run **lintian** (Debian-policy linter),
with **piuparts** (install/remove test in a chroot) and **autopkgtest**
(`debian/tests`) as further gates the wiki's build script chains together. All
three are effectively required before Debian will accept a package.

### Build-area export & overlay

`--git-export-dir=DIR` builds a clean checkout of a chosen `--git-export`
treeish (`HEAD`, `INDEX`, or `WC` for the working copy). `--git-overlay`
combines a debian-only branch with a separately-supplied upstream tarball —
useful when you keep *only* `debian/` in git.

## 6. `gbp pq` — the git-native patch queue (quilt alternative)

`gbp pq` manages `debian/patches/` "without any need for GNU Quilt" by
representing each patch as a **git commit** on a temporary
`patch-queue/<branch>` branch. The round-trip:

```sh
gbp pq import          # read debian/patches/series → commits on patch-queue/debian/sid
# … now edit source with normal git: commit, amend, rebase, reorder …
gbp pq export          # regenerate debian/patches/* + series from those commits
```

Actions (from `man gbp-pq`):

- **`import`** — build the patch-queue branch from the current series (patches
  must apply without fuzz).
- **`export`** — write the commits back out as the numbered patch series.
  `--commit` also commits the regenerated `debian/patches/`, `--drop` deletes
  the temp branch afterward. Common one-liner: `gbp pq export --drop --commit`.
- **`rebase`** — rebase the patch queue onto the current branch (e.g. after a
  new upstream import); `--time-machine=N` walks back commits to find where the
  queue still applies.
- **`switch`** — toggle between the base branch and its patch-queue branch.
- **`drop`** — delete the patch-queue branch.

### Commit-message tags → patch metadata

`gbp pq export` reads special trailers in each commit message:

- `Gbp-Pq: Name <file>` — the exported patch filename (else auto-numbered via
  `--patch-num-format`, default `%04d-`).
- `Gbp-Pq: Topic <dir>` — group the patch into `debian/patches/<dir>/` (e.g.
  separating upstream backports from Debian-specific changes).
- `Gbp-Pq: Ignore` — a queue commit that should *not* become a patch.

This is the git-workflow counterpart to quilt's `quilt header --dep3`. Choose
**one** front-end per package and stick with it: editing `debian/patches`
directly with quilt and via `gbp pq` at the same time causes drift. Both emit
the identical `3.0 (quilt)` series that `DEBIAN.md`/`dpkg-source` consume.

## 7. Publish stage

Set the changelog distribution (from `UNRELEASED` to e.g. `unstable`), commit,
then:

```sh
gbp buildpackage --git-tag           # build + create a debian/X.Y-Z tag
```

`--git-tag` tags the release commit (format `--git-debian-tag`, default
`debian/%(version)s`); `--git-sign-tags` + `--git-keyid` PGP-sign it. Push to
Salsa and go through sponsorship for Debian proper. The wiki then advises a
throwaway `gbp dch --snapshot` commit so an accidental push fails the CI gates
harmlessly rather than releasing.

## 8. How this maps to the project's goals

For **reduced dependencies / further patches / overridden install paths**, the
git workflow doesn't change *what* you edit (that's still `debian/control`,
`debian/rules` overrides, and `debian/patches` — see `DEBIAN.md`), it changes
*how you author and build*:

- Keep the whole modified package on a `debian/*` branch; upstream stays
  pristine on `upstream/*` + `pristine-tar`, so your deltas are auditable.
- Add "further patches" as `gbp pq` commits (or quilt patches) — either way they
  land in `debian/patches/series`.
- Iterate with `gbp buildpackage --git-pbuilder` for reproducible, host-clean
  builds; gate on lintian/piuparts before trusting the result.
- The reduced-dep / path-override edits are ordinary commits on the debian
  branch; nothing about gbp constrains them.

## 9. Quick command reference

| task                              | command                                             |
|-----------------------------------|-----------------------------------------------------|
| import a new upstream release     | `gbp import-orig --uscan`                            |
| show effective config value       | `gbp config DEFAULT.debian-branch`                  |
| add/edit changelog entry          | `dch --new-version …` / `dch --edit` / `gbp dch`    |
| pull patches into git commits     | `gbp pq import`                                     |
| write commits back to patches     | `gbp pq export --drop --commit`                     |
| build in a clean chroot           | `gbp buildpackage --git-pbuilder --git-dist=sid`    |
| build source-only package         | `gbp buildpackage -S -d`                            |
| build + tag a release             | `gbp buildpackage --git-tag`                        |
| export tree without building      | `gbp buildpackage --git-export-dir=… --git-builder=/bin/true --git-no-pbuilder --git-no-hooks --git-no-purge` |
