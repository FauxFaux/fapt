# quilt — Managing a Patch Stack

`quilt` is the tool that sits underneath Debian's `3.0 (quilt)` source format.
`DEBIAN.md` §7 introduced *why* Debian keeps its changes as a numbered patch
series in `debian/patches/`; this document explains the tool that creates and
maintains that series. Source: `man 1 quilt` (and the sway example, whose
`debian/patches/` is a real quilt series).

Note: on a `3.0 (quilt)` package, `dpkg-source`/`dpkg-buildpackage` **apply the
series automatically** at unpack/build time — you do not need quilt to *build*.
You need quilt (or `gbp pq`, see `GIT-PACKAGING.md`) to *author and edit* the
patches.

---

## 1. The core idea: a stack of patches over a pristine tree

Your "primary working material is patches." quilt maintains a **stack** of
patches applied incrementally on top of the pristine upstream tree:

```
        ┌─ 0006-revert_drag_lock…       ← top of stack (quilt top)
        ├─ 0005-xdg-shell-fix…
        ├─ 0004-feat-comment…
        ├─ 0002-Modify-default-config…
        ├─ 0001-Install-zsh-completions…
  base ─┴─ pristine upstream source
```

Patches are **pushed** onto the stack and **popped** off it. Most commands act
on the *topmost* patch by default. Two files/dirs are the whole state model:

- **`patches/`** — the patch files themselves, plus `patches/series` listing
  them **in application order** (one filename per line; `#` comments allowed;
  per-patch `-pN` options can follow the name). This is what you ship and commit.
- **`.pc/`** — quilt's private bookkeeping. Before applying a patch, quilt
  copies every file that patch will touch into `.pc/<patch-name>/`, and records
  the currently-applied set in `.pc/applied-patches`. `quilt refresh` later
  diffs those backups against the working tree to regenerate the patch. You do
  **not** hand-edit `.pc/`; it is auto-managed and safe to leave alone (or
  `.gitignore`).

`QUILT_PATCHES` / `QUILT_SERIES` env vars override the default locations.
Debian sets `QUILT_PATCHES=debian/patches` (commonly via `~/.quiltrc` or
`export` in the shell) so quilt operates on the Debian series from the source
root.

## 2. The authoring loop: new → add/edit → refresh

Creating a new Debian patch (the mechanical equivalent of adding a file to
`debian/patches/` + `series`):

```sh
quilt push -a                 # apply the whole existing series first
quilt new 0007-fix-foo.patch  # create empty patch, insert ABOVE current top, add to series
quilt add path/to/file.c      # tell quilt this file will be edited (MUST precede editing)
$EDITOR path/to/file.c        # make the change
quilt refresh                 # write the accumulated diff into 0007-fix-foo.patch
```

The non-obvious rule: **`quilt add` before you modify a file.** quilt can only
capture a change if it snapshotted the file (into `.pc/`) beforehand. `quilt
edit file` is the shortcut — it does `add` then opens `$EDITOR` in one step.

`quilt refresh` is what actually (re)writes the patch file from the current
working-tree state. Nothing is written to the `.patch` file until you refresh.

## 3. Moving through the stack

| command                | effect                                                        |
|------------------------|---------------------------------------------------------------|
| `quilt push [n\|patch]`| apply next patch / n patches / up to named patch              |
| `quilt push -a`        | apply the entire series                                       |
| `quilt pop [n\|patch]` | unapply topmost / n / down to named patch                     |
| `quilt pop -a`         | unapply everything, back to pristine tree                     |
| `quilt top`            | name of the currently-applied top patch                       |
| `quilt series`         | the whole series, in order                                    |
| `quilt applied` / `unapplied` | which patches are / aren't currently on the stack      |
| `quilt next` / `previous` | neighbours of the top patch in the series                  |

`push`/`pop` accept `--refresh` to auto-refresh patches as they move. `push`
has `--fuzz=N`, `-f` (force, leaving `.rej` files), and `-m` (merge) for
patches that no longer apply cleanly — the usual situation after a new upstream
version.

## 4. Editing an existing patch (not on top)

To change a patch buried in the stack, pop up to it so it becomes top, edit,
refresh, then push the rest back:

```sh
quilt push 0004-feat-comment…   # make 0004 the top patch
quilt edit config.in            # add + edit
quilt refresh                   # rewrite 0004
quilt push -a                   # re-apply the patches above it
```

`quilt refresh -f` *can* refresh a non-top patch, but it warns about files
"shadowed" by later patches and silently ignores their changes — popping to the
patch is the safe way.

## 5. Importing, deleting, reordering

- **`quilt import [-p N] file.patch`** — pull an external patch into the series
  (inserted after the current top; must then be `push`ed to apply). This is how
  you bring in an upstream backport, e.g. sway's
  `0005-xdg-shell-fix-reported-WM-capabilities.patch`.
- **`quilt delete [-r] [patch]`** — remove a patch from `series` (and, with
  `-r`, delete the file). Pops it first if applied.
- **`quilt rename -P old new`**, **`quilt fork`** — rename / duplicate a patch.
- **Reorder** by editing `patches/series` directly (allowed as long as the
  currently-applied prefix keeps its order).

## 6. Patch metadata and provenance

- **`quilt header [-e|-a|-r] [--dep3]`** — view/edit the free-text header stored
  *inside* the patch file (everything before the actual diff). `--dep3` inserts
  a [DEP-3](http://dep.debian.net/deps/dep3/) template — the standard
  `Description:` / `Origin:` / `Bug:` / `Forwarded:` headers Debian expects so
  reviewers know where a patch came from and whether it's upstreamable. Sway's
  patches carry `git format-patch`-style headers, which are DEP-3-compatible.
- **`quilt refresh --no-timestamps --no-index`** — produce clean, reproducible
  patch headers (no volatile timestamps / `Index:` lines). Debian maintainers
  typically set these in `~/.quiltrc` to keep diffs minimal.
- **`quilt diff [-z] [-P patch]`** — preview the diff of the top (or named)
  patch; `-z` shows only what's changed since the last refresh.
- **`quilt files`** / **`quilt annotate file`** — which files a patch touches /
  which patch last touched each line.

## 7. Recommended `~/.quiltrc` for Debian work

Almost every Debian guide sets these so quilt targets `debian/patches` and
produces tidy patches:

```sh
QUILT_PATCHES=debian/patches
QUILT_REFRESH_ARGS="-p ab --no-timestamps --no-index"
QUILT_DIFF_ARGS="--no-timestamps --no-index"
QUILT_NO_DIFF_TIMESTAMPS=1
```

`-p ab` makes patches use `a/file` … `b/file` paths (git style) rather than
quilt's default `dir.orig/file`.

## 8. Relation to the rest of the toolchain

- The **series quilt maintains** is exactly the `debian/patches/series` that
  `DEBIAN.md` describes and that `dpkg-source` applies during a `3.0 (quilt)`
  build. quilt is the *authoring* front-end; the build system is the *consumer*.
- **`gbp pq`** (see `GIT-PACKAGING.md`) is an alternative front-end that lets you
  edit the same series as *git commits* on a throwaway `patch-queue/` branch,
  "without any need for GNU Quilt." Same output (`debian/patches/*` + `series`),
  different editing UX. A maintainer who lives in git usually prefers `gbp pq`;
  quilt is the lingua franca that always works and needs no branch setup.
- For **this project's "further patches" goal**: either add the patch with
  `quilt new … / edit / refresh`, or drop a ready-made `.patch` in with
  `quilt import`, then commit `debian/patches/`. No build-script edit needed.
