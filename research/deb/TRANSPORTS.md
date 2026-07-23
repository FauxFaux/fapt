# Writing an APT Transport (Acquire Method)

A guide for implementors who need to teach `apt` how to fetch from a new
kind of location (a new URI scheme). This is what packages like
`apt-transport-s3`, `apt-transport-tor`, or the built-in `http`/`file`/`cdrom`
handlers are.

Everything below is derived from the apt 3.2.0 source and the upstream
`apt-transport-s3` 2.2.1 reference implementation. Authoritative sources:

- `doc/method.dbk` — the official "APT Method Interface" specification.
- `apt-pkg/acquire-method.{h,cc}` — the C++ base class methods link against.
- `apt-pkg/acquire-worker.cc` — the apt-side of the protocol (how apt drives a method).
- `apt-pkg/acquire.cc` — URI → queue → method-config mapping.
- `methods/aptmethod.h`, `methods/file.cc`, `methods/copy.cc` — small in-tree methods.
- `apt-transport-s3-2.2.1/s3` — a complete method written in Python (no libapt link).

---

## 1. What a transport actually is

A transport (apt calls it a **method**) is a **standalone executable** — one
per URI scheme — that apt runs as a **child process** and talks to over
**stdin/stdout** using a line-oriented, RFC822-style text protocol. stderr is
reserved for emergency error reporting.

Key consequences:

- **You can write a method in any language.** apt does not `dlopen()` anything;
  it `fork`/`exec`s a binary and pipes messages to it. The s3 method is a Python
  script; the in-tree methods are C++ that link `libapt-pkg`, but that is a
  convenience, not a requirement.
- **One method process serves one URI scheme.** The scheme in a
  `sources.list` line (`s3://…`, `https://…`) selects which binary apt launches.
- apt may run **several instances concurrently** (and pipeline requests to a
  single instance) unless you declare otherwise via capabilities.

There is a **one-to-one mapping of URI access-scheme → method binary**
(`doc/method.dbk`, "Terms"/"method").

---

## 2. Discovery, naming and loading

When apt needs a URI whose scheme is `X`, the worker resolves the binary like
this (`apt-pkg/acquire-worker.cc:96-120`, `Worker::Start`):

```
confItem = "Dir::Bin::Methods::" + X          // e.g. Dir::Bin::Methods::s3
if config has confItem:
    Method = FindFile(confItem)                // explicit path override
else:
    Method = FindDir("Dir::Bin::Methods") + X  // default dir + scheme name
```

So by default:

| sources.list scheme | binary apt executes                |
|---------------------|------------------------------------|
| `s3://…`            | `/usr/lib/apt/methods/s3`          |
| `https://…`         | `/usr/lib/apt/methods/https`       |
| `foo+bar://…`       | `/usr/lib/apt/methods/foo+bar`     |

- **Default directory** is `Dir::Bin::Methods`, which is
  `/usr/lib/apt/methods/` on Debian/Ubuntu. Your binary's **filename is the
  scheme name**, verbatim, and must be **executable**.
- **Per-scheme override:** setting `Dir::Bin::Methods::s3 "/path/to/mybinary";`
  in apt config points that one scheme at an arbitrary path. Useful for testing
  without installing to `/usr/lib/apt/methods`.
- **Disabling a scheme:** if the resolved path is the literal string `false`,
  apt reports *"The method '%s' is explicitly disabled via configuration."*
  (`acquire-worker.cc:104-112`).
- **Missing binary:** apt prints *"The method driver %s could not be found"* and
  suggests installing the package named `apt-transport-<scheme>` — where
  `<scheme>` is truncated at the first `+` (`acquire-worker.cc:113-118`). This is
  why the convention is to ship your method in a package called
  `apt-transport-<scheme>`.

There is no registry file and no plugin manifest. **Dropping an executable of
the right name into the methods directory is the entire installation.**

**`argv[0]` is always the scheme name.** apt execs the binary with `argv[0]`
set to `<methodsdir>/<scheme>` even when `Dir::Bin::Methods::<scheme>` points at a
different path (`acquire-worker.cc:120,153`). This is how the in-tree `store`
method dispatches: `gzip`, `bzip2`, `xz`, … are symlinks to `store`, which reads
`flNotDir(argv[0])` to decide which compressor to run. You can use the same
trick to serve several related schemes from one binary via symlinks.

**Exit signalling.** apt signals "no more work" by **closing your stdin** (EOF);
your read loop must exit cleanly on EOF. Unless you set `Needs-Cleanup`, apt will
also `SIGINT` the process during teardown (`acquire-worker.cc:76-89`).

### `+scheme` composition

A scheme may contain a `+` (e.g. `tor+http`, `mirror+file`). apt splits on it in
a few places (package-name suggestion above; config namespacing below). Choose a
plain scheme name unless you are deliberately layering on top of another method.

---

## 3. The wire protocol

### 3.1 Framing

- Communication is a stream of **messages**. A message is **a header line
  followed by zero or more RFC822 field lines, terminated by one blank line**
  (`doc/method.dbk` §2.1; s3 `_read_message`, `s3:476-505`).
- The **header line** is `NNN Human Text` — a 3-digit status code and a
  free-text label (the label is informational; apt parses the number).
- Field lines are `Key: Value`. Keys allow only space, `-`, and alphanumerics;
  values allow printable ASCII, UTF-8, `\n` and `\t`
  (`acquire-method.cc:91-102`, `SendMessage`).
- **Multi-line values** are folded RFC822-style: continuation lines begin with a
  leading space. apt writes them by joining on `"\n "` (`acquire-method.cc:128-134`).
- **Always flush after the trailing blank line.** apt reads with `select()`;
  a message that isn't flushed will hang the transfer. (The s3 script achieves
  this with `python3 -u`; C++ `SendMessage` ends with `std::flush`.)
- **Security:** apt *rejects messages containing control characters* and aborts
  (`acquire-method.cc:104-109`); redirect targets are checked against a strict
  character whitelist (`acquire-method.cc:527-532`). Never emit raw control
  bytes.

### 3.2 Status codes

Digit conventions mirror HTTP: `1xx` informational, `2xx` success, `3xx`
intermediate, `4xx` failure, `6xx` messages **to** the method
(`doc/method.dbk` §2.2).

**Method → apt** (you send these):

| Code | Name                      | Meaning                                         |
|------|---------------------------|-------------------------------------------------|
| 100  | Capabilities              | Announce what the method supports (send first)  |
| 101  | Log                       | Debug log line (shown only when debugging)      |
| 102  | Status                    | Progress / pre-transfer status text             |
| 103  | Redirect                  | URI redirected to a new URI                     |
| 104  | Warning                   | Non-fatal warning (`aptmethod.h:463`)           |
| 105  | Audit                     | Audit message (`aptmethod.h:467`)               |
| 200  | URI Start                 | Transfer of a URI has begun                     |
| 201  | URI Done                  | Transfer finished successfully                  |
| 351  | Aux Request               | Ask apt to fetch a helper file for you          |
| 400  | URI Failure               | This URI could not be fetched                   |
| 401  | General Failure           | Fatal method error; method should then exit     |
| 402  | Authorization Required    | Need user/password; wait for a 602              |
| 403  | Media Failure             | Need a media/disc change; wait for a 603        |

**apt → method** (you receive and must handle these):

| Code | Name                       | Meaning                                    |
|------|----------------------------|--------------------------------------------|
| 600  | URI Acquire                | Please fetch this URI to this Filename     |
| 601  | Configuration              | Here is apt's configuration space          |
| 602  | Authorization Credentials  | Reply to your 402 (User/Password)          |
| 603  | Media Changed              | Reply to your 403                          |

Rules (`doc/method.dbk` §2.2):

- **Only 6xx is sent to the method; the method must never emit a 6xx.**
- apt may send `600` messages **at any time and without waiting** for you to
  finish previous ones — you must be able to queue an unbounded number.
- After sending `351`, `402`, or `403` you must **keep reading** further 6xx
  messages until the matching `600`/`602`/`603` arrives.

### 3.3 Common header fields

From `doc/method.dbk` §2.3 and the senders in `acquire-method.cc`:

| Field                | Direction | Used in / meaning                                        |
|----------------------|-----------|----------------------------------------------------------|
| `URI`                | both      | The URI a message is about                               |
| `Filename`           | both      | Destination path in the local filesystem                 |
| `Last-Modified`      | both      | RFC1123 timestamp, for If-Modified-Since                 |
| `IMS-Hit`            | → apt     | The cached file is still valid (no download needed)      |
| `Size`               | → apt     | File size in bytes                                        |
| `Resume-Point`       | → apt     | Byte offset a resumed transfer started at                |
| `Maximum-Size`       | apt →     | Reject the download if it exceeds this (600)             |
| `Expected-<Hash>`    | apt →     | Expected hash (e.g. `Expected-SHA256`) for the item      |
| `<Hash>-Hash`        | → apt     | Computed hash, e.g. `SHA256-Hash`, `SHA512-Hash`, `MD5-Hash` |
| `Message`            | → apt     | Human-readable text (status / failure reason)            |
| `FailReason`         | → apt     | Machine-readable failure tag (e.g. `HttpError404`)       |
| `Transient-Failure`  | → apt     | Failure is retriable, not permanent                      |
| `Index-File`         | apt →     | This item is an index (Packages/Release), not a .deb     |
| `Fail-Ignore`        | apt →     | A failure here is non-fatal to the overall run           |
| `Config-Item`        | apt →     | One `key=value` entry of apt config (601)                |
| `Site` / `User` / `Password` | both | Authorization (402/602)                              |
| `Media` / `Drive`    | both      | Removable-media handling (403/603)                       |
| `New-URI`            | → apt     | Redirect target (103)                                    |
| `Alt-*`              | → apt     | Info about an alternate local file (e.g. `Alt-Filename`) |
| `UsedMirror`         | → apt     | Which mirror was actually used                           |

Hash fields are emitted for every hash type apt knows about; note the legacy
alias: `MD5Sum` is *also* sent as `MD5-Hash` for backward compatibility
(`acquire-method.cc:236-244`).

---

## 4. Lifecycle / handshake

The canonical flow (`doc/method.dbk` §2.2; s3 `run()`, `s3:529-546`):

```
method                          apt
  |  100 Capabilities   ───────► |   (method sends this immediately on startup)
  |  ◄─────────────── 601 Configuration   (only if you set Send-Config)
  |  ◄─────────────── 600 URI Acquire      (one per file, may arrive in bursts)
  |  102 Status        ───────► |          (optional progress)
  |  200 URI Start     ───────► |          (optional; size/mtime known)
  |  201 URI Done      ───────► |          (success: filename + hashes)
  |       …or…                  |
  |  400 URI Failure   ───────► |          (this URI failed)
  |  ◄─────────────── (more 600s / EOF)
```

1. **On startup, send `100 Capabilities` first.** apt parses it in
   `Worker::Capabilities` (`acquire-worker.cc:771-799`).
2. If you advertised `Send-Config`, apt sends `601 Configuration` before the
   first acquire (`acquire-worker.cc:850-895`).
3. apt then streams `600 URI Acquire`. For each, do the fetch and reply with
   `200`/`201` on success or `400` on failure.
4. apt closes stdin (EOF) when there is no more work; your read loop should
   treat EOF as "exit 0" (s3 `_read_message` returns `None` → `run` returns 0).

---

## 5. Capabilities (the `100` message)

Advertised via fields on the `100 Capabilities` message. In C++ these are flags
passed to the `pkgAcqMethod` constructor and translated to fields
(`acquire-method.cc:53-81`); a script just prints the fields directly (s3
`send_capabilities`, `s3:511-515`). Meaning of each
(`acquire-worker.cc:771-799`, `doc/method.dbk` §2.3):

| Field              | C++ flag         | Meaning                                                       |
|--------------------|------------------|---------------------------------------------------------------|
| `Version`          | ctor arg         | Method version string (required)                              |
| `Single-Instance`  | `SingleInstance` | Run **one** process for all URIs of this scheme, not many     |
| `Pipeline`         | `Pipeline`       | You accept many outstanding requests at once (HTTP-style)     |
| `Send-Config`      | `SendConfig`     | Ask apt to send you `601 Configuration`                       |
| `Local-Only`       | `LocalOnly`      | You only ever return `Filename:` (no network); affects UI     |
| `Needs-Cleanup`    | `NeedsCleanup`   | Keep the process alive while returned files are in use (CD/file) |
| `Removable`        | `Removable`      | Deals with removable media                                    |
| `AuxRequests`      | `AuxRequests`    | You may emit `351 Aux Request`                                |
| `Send-URI-Encoded` | `SendURIEncoded` | You want URIs delivered percent-encoded (see §9)              |

A typical **network** method (like s3) sends:

```
100 Capabilities
Version: 1.1
Single-Instance: true
Send-Config: true

```

`file`/`copy` add `Local-Only`/`SendURIEncoded`; `http` adds `Pipeline`.

---

## 6. Handling `600 URI Acquire`

This is the core request. Fields apt puts on it (`acquire-method.cc:420-445`):

- `URI` — what to fetch (in your scheme).
- `Filename` — where to write it. **Write the bytes here.**
- `Last-Modified` — if present, the mtime of the copy apt already has; use it for
  a conditional/If-Modified-Since request.
- `Index-File: true` — this is an index file (metadata), not a package.
- `Fail-Ignore: true` — a failure is tolerable.
- `Expected-<Hash>` — expected hashes; `Maximum-Size` — hard size cap.

Your job:

1. (Optional) `102 Status` while connecting/waiting for headers.
2. (Optional) `200 URI Start` with `Size` and `Last-Modified` once known.
3. Stream the body to `Filename`, computing hashes as you go.
4. On success send `201 URI Done` with `URI`, `Filename`, `Size`,
   `Last-Modified`, and the hash fields (`SHA256-Hash`, `SHA512-Hash`,
   `MD5-Hash`, …). apt verifies these against `Expected-*`.
5. On failure send `400 URI Failure` with `URI`, `Message`, and ideally a
   `FailReason`. Set `Transient-Failure: true` if a retry might succeed.

**If-Modified-Since:** if the remote object is unchanged since `Last-Modified`,
send `201 URI Done` with `IMS-Hit: true` and no body — apt keeps its cached copy
(`file.cc:78-83`, `acquire-method.cc:272-273`).

Minimal successful reply (s3 `send_uri_done`, `s3:667-675`):

```
201 URI Done
URI: s3://bucket/dists/stable/Release
Filename: /var/lib/apt/lists/partial/…Release
Size: 12345
Last-Modified: Tue, 01 Jul 2026 10:00:00 GMT
MD5-Hash: …
SHA256-Hash: …
SHA512-Hash: …

```

---

## 7. Configuration (`601`) and options

If you set `Send-Config`, apt sends the **entire config space** as a series of
`Config-Item: key=value` lines (`acquire-worker.cc:850-895`; parsed by
`pkgAcqMethod::Configuration`, `acquire-method.cc:346-376`). Values are
DeQuoted. You filter out what you care about.

Conventions:

- Method-specific options live under **`Acquire::<scheme>::…`**. The C++ helper
  `ConfigFind*` (`aptmethod.h:76-104`) looks them up for you; a script scans the
  `Config-Item` list (s3 `configure`, `s3:548-559`, which harvests
  `Acquire::http::Proxy`).
- Proxy: honor `Acquire::http::Proxy` / `Acquire::https::Proxy` if you speak
  HTTP under the hood (s3 exports them into `http_proxy`/`https_proxy`).
- Debug: apt enables `Debug::Acquire::<scheme>` — the C++ base wires this to
  `DebugEnabled()` (`aptmethod.h:60-75`).

Users configure these in `/etc/apt/apt.conf.d/*.conf`, e.g.:

```
Acquire::http::Proxy "http://proxy:3128/";
Dir::Bin::Methods::s3 "/usr/local/bin/my-s3";     // point a scheme at your binary
Debug::Acquire::s3 "true";
```

Note apt always injects `Acquire::Send-URI-Encoded=1` unless the user set it
(`acquire-worker.cc:862-863`).

---

## 8. Authentication

Two layers, use whichever fits:

**(a) Interactive 402/602 (legacy).** Send `402 Authorization Required` with a
`Site`, then keep reading until apt returns `602 Authorization Credentials`
carrying `Site`/`User`/`Password` (`doc/method.dbk` §2.2). This round-trip still
exists in the docs, but modern apt resolves credentials *inside* the method via
`auth.conf` rather than round-tripping — `402` is not even in the worker's
current `MessageType` enum (`acquire-worker.cc:195-209`). Prefer (b) or (c).

**(b) `auth.conf` (recommended for repos).** apt ships a netrc-style credential
store at `/etc/apt/auth.conf` and `/etc/apt/auth.conf.d/*.conf`
(`doc/apt_auth.conf.5.xml`):

```
machine example.org/debian login apt password s3cr3t
```

The C++ base class **`aptAuthConfMethod`** (`aptmethod.h:538-613`) loads these
files (respecting `Dir::Etc::netrc` / `Dir::Etc::netrcparts`) and offers
`MaybeAddAuthTo(URI&)`, which fills in `User`/`Password` from the best-matching
`machine` entry (longest host+path match wins). The `http` method subclasses
this. If you write in C++, subclass `aptAuthConfMethod` to get auth.conf for
free; if you write a script, parse these files yourself.

> **Encryption requirement:** a `machine` entry with **no** `protocol://`
> prefix only matches encrypted schemes (https / tor+https). If it would match
> an unencrypted transport apt refuses the credentials and warns *"Credentials …
> match, but the protocol is not encrypted. Annotate with `<scheme>://`."*
> (`apt-pkg/contrib/netrc.cc:113-120`). If your scheme is effectively secure
> (e.g. s3 over TLS), tell users to annotate: `machine myscheme://host/… login …`.

**(c) Out-of-band credentials.** The s3 method reads its own
`/etc/apt/s3auth.conf`, environment variables (`AWS_ACCESS_KEY_ID`, …), and the
EC2 instance-metadata service for IAM roles (`s3:141-278`). This is the simplest
route when the credential model is provider-specific.

---

## 9. Details that bite

- **URI encoding.** Modern apt sets `Send-URI-Encoded` and delivers URIs
  percent-encoded; you must decode the parts you use (`aptmethod.h:510-515`,
  `DecodeSendURI`; `file.cc:51`). If you *don't* advertise the capability apt may
  still hand you encoded URIs depending on `Acquire::Send-URI-Encoded` — handle
  both. When emitting URIs back, encode with `URIEncode` semantics
  (`aptmethod.h:504-508`; note apt encodes `+` as a workaround for an S3/LP bug).
- **Concurrency & Single-Instance.** Without `Single-Instance`, apt may spawn
  multiple copies of your binary; make sure that's safe. Even a single instance
  may receive pipelined `600`s before finishing earlier ones.
- **Redirects.** Emit `103 Redirect` with `URI` + `New-URI`; apt re-queues the
  new URI (possibly to a *different* method) and detects redirect loops
  (`acquire-method.cc:525-538`, `acquire-worker.cc:300-370`). Targets are
  charset-validated.
- **Aux requests (`351`).** With `AuxRequests`, a method can ask apt to fetch a
  helper file via another method; apt returns it as a `600 URI Acquire` whose
  `Filename` exists on success (`doc/method.dbk` §2.3; fields `Aux-URI`,
  `MaximumSize`, `Aux-ShortDesc`, `Aux-Description`).
- **Failure vs. exit.** `400 URI Failure` fails one URI and continues; `401
  General Failure` means the method is broken and should exit. Prefer `400` with
  a good `Message`/`FailReason` for per-file problems. For rate limiting, set
  `FailReason: TooManyRequests` and/or a `Retry-After` (seconds) field and apt
  will back off and retry (`acquire-worker.cc:706-720`) — useful for S3-style
  `503 SlowDown`.
- **Never write to stdout except protocol messages.** Any stray text corrupts
  the stream. Send diagnostics via `101 Log` / `102 Status`, or to stderr.

---

## 10. Sandboxing & privileges

apt runs methods with reduced privileges and, optionally, seccomp:

- **Privilege drop.** `DropPrivsOrDie()` drops to the `APT::Sandbox::User`
  (default `_apt`, `init.cc:179`) after config is received
  (`aptmethod.h:131`, `acquire-method.cc:206-212`). Your method therefore runs as
  an unprivileged user — it can write to the `Filename` apt gave you and read
  world-readable config, but **not** to root-owned paths. `auth.conf` files are
  read *before* the drop by `aptAuthConfMethod`.
- **Seccomp.** C++ methods can set `SeccompFlags` (`BASE`, `NETWORK`,
  `DIRECTORY`) to opt into a syscall allowlist enforced when
  `APT::Sandbox::Seccomp` is on (`aptmethod.h:110-439`). Network methods must set
  `NETWORK`. This applies to the in-tree C++ methods; script-based methods run
  under the same privilege drop but are not seccomp-filtered by this mechanism.
- Implication for scripts: your interpreter and its libraries must be reachable
  and the script must function as `_apt`. Test as that user.

---

## 11. Two ways to implement

### A. C++ linking `libapt-pkg` (in-tree style)

Subclass `aptMethod` (or `aptAuthConfMethod`), override `URIAcquire`, and call
the helper senders. The base class does the read loop, capability message,
config parsing, privilege drop, hashing helpers, and IMS bookkeeping. The whole
of `methods/file.cc` (149 lines) is a working example; the skeleton is:

```cpp
#include "aptmethod.h"

class MyMethod final : public aptMethod {
   bool URIAcquire(std::string const &Message, FetchItem *Itm) override {
      URI Get(Itm->Uri);
      FetchResult Res;
      Res.Filename = Itm->DestFile;
      // 1. fetch Get into Itm->DestFile, honoring Itm->LastModified for IMS
      URIStart(Res);                    // -> 200
      // 2. write bytes; then:
      CalculateHashes(Itm, Res);        // fills Res.Hashes from the file
      URIDone(Res);                     // -> 201  (or Fail("msg") -> 400)
      return true;
   }
public:
   MyMethod() : aptMethod("myscheme", "1.0", SingleInstance | SendConfig) {
      SeccompFlags = aptMethod::BASE | aptMethod::NETWORK;   // if networked
   }
};

int main() { return MyMethod().Run(); }
```

`Run()` (`acquire-method.cc:383-465`) is the message engine: it dispatches `601`
to `Configuration()` and each `600` to your `URIAcquire()`.

### B. Any language, protocol by hand (s3 style)

No apt headers. Implement the read loop, print the messages. The complete shape
is in `apt-transport-s3-2.2.1/s3`; a minimal skeleton:

```python
#!/usr/bin/python3 -u
import sys, hashlib

def send(code, name, fields):
    out = f"{code} {name}\n"
    for k, v in fields.items():
        if v is not None:
            out += f"{k}: {v}\n"
    sys.stdout.write(out + "\n")          # blank line terminates; -u flushes

def read_message():
    line = sys.stdin.readline()
    while line == "\n":
        line = sys.stdin.readline()
    if not line:
        return None
    msg = {"_number": int(line.split(" ", 1)[0])}
    while True:
        line = sys.stdin.readline()
        if line in ("", "\n"):
            break
        k, v = line.split(":", 1)
        msg.setdefault(k, []).append(v.strip())
    return msg

send(100, "Capabilities", {"Version": "1.0", "Single-Instance": "true",
                           "Send-Config": "true"})
while (m := read_message()) is not None:
    if m["_number"] == 601:
        pass                              # read Config-Item entries
    elif m["_number"] == 600:
        uri, dest = m["URI"][0], m["Filename"][0]
        try:
            data = fetch(uri)             # your transport logic
            with open(dest, "wb") as f:
                f.write(data)
            send(201, "URI Done", {
                "URI": uri, "Filename": dest, "Size": len(data),
                "SHA256-Hash": hashlib.sha256(data).hexdigest(),
                "SHA512-Hash": hashlib.sha512(data).hexdigest(),
                "MD5-Hash":    hashlib.md5(data).hexdigest()})
        except Exception as e:
            send(400, "URI Failure", {"URI": uri, "Message": str(e)})
```

---

## 12. Packaging & installation

The whole install is "put the executable in the methods dir." From
`apt-transport-s3`'s `debian/install`:

```
s3 /usr/lib/apt/methods/
```

Package conventions (`apt-transport-s3` `debian/control`):

- Name the binary package **`apt-transport-<scheme>`** (matches apt's
  "is package installed?" hint).
- `Depends: apt` plus your runtime (e.g. `python3`, `python3-configobj`).
- `Architecture: all` is fine for a script.
- Ship a manpage `apt-transport-<scheme>.8` and document the sources.list syntax.

Users then add a line like:

```
deb s3://my-bucket/repo stable main
```

---

## 13. Implementor checklist

- [ ] Binary named exactly `<scheme>`, executable, installed to
      `/usr/lib/apt/methods/`.
- [ ] Sends `100 Capabilities` **first thing**, with a `Version`.
- [ ] Reads `600`/`601` in a loop; treats stdin EOF as clean exit.
- [ ] Writes bytes to the `600`'s `Filename`; replies `201` with `Size` +
      hashes, or `400` with `Message`/`FailReason`.
- [ ] Honors `Last-Modified` for IMS (`IMS-Hit: true`) to avoid re-downloads.
- [ ] Emits a **blank line + flush** after every message.
- [ ] Emits no non-protocol output on stdout (use `101`/`102`/stderr).
- [ ] Decodes percent-encoded URIs (`Send-URI-Encoded`).
- [ ] Works when running as the unprivileged `_apt` user.
- [ ] Auth via `auth.conf.d` (or 402/602), not hard-coded secrets.
- [ ] Sets `NETWORK` seccomp flag if C++ and networked.

## 14. Testing without installing

A method is just a program on a pipe, so you can drive it by hand
(`apt-transport-s3` README, "Testing"):

```
$ printf '601 Configuration\n\n600 URI Acquire\nURI: myscheme://host/path\nFilename: /tmp/out\n\n' \
    | /usr/lib/apt/methods/myscheme
```

Point apt at an uninstalled build with
`-o Dir::Bin::Methods::myscheme=/path/to/binary`, and watch the exchange with
`-o Debug::Acquire::myscheme=true`. Run a real fetch end-to-end with:

```
$ sudo apt-get -o Debug::Acquire::myscheme=true update
```
