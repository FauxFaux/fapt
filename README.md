`apt`, the Debian package manager, doesn't like giving up its data.

`apt-dump` dumps that data out as a [capnproto](https://capnproto.org/)
stream, for use in other applications.

```bash
sudo apt install libcapnp-dev capnproto libapt-pkg-dev cmake build-essential
mkdir build
cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
make
./apt-dump raw-sources | capnp decode ../apt.capnp RawSource
```

This will generate the data stream, then use `capnp` to parse it
back to a human readable format, like:
```
( package = "0ad",
  version = "0.0.21-2",
  index = "http://deb.debian.org/debian sid/main Sources",
  binaries = ["0ad"],
  files = [
    ( name = "pool/main/0/0ad/0ad_0.0.21-2.dsc",
      size = 2363,
      md5 = "5f2af935f4537ede6169db8946d18d81",
      sha256 = "ee98572de81be0ffbf039951111fdef3a431d81892481a959363fbb93cfb780e" ),
    ( name = "pool/main/0/0ad/0ad_0.0.21.orig.tar.xz",
      size = 29196476,
      md5 = "095eade8c9b3deaf25d0d7fa423ff860",
      sha256 = "96be23e4284a3931ef9536f988f2517040bde1f8700ee048bff18c932d8683cf" ),
    ( name = "pool/main/0/0ad/0ad_0.0.21-2.debian.tar.xz",
      size = 71420,
      md5 = "01d28e643619455fef8d40f1d1e7da7d",
      sha256 = "2f6e5b751872932971c4dbf618c32ddef1021f195d0457f57030b814cb1749c7" ) ],
  entries = [
    ( key = "Maintainer",
      value = "Debian Games Team <pkg-games-devel@lists.alioth.debian.org>" ),
    ( key = "Uploaders",
      value = "Vincent Cheng <vcheng@debian.org>" ),
    ( key = "Build-Depends",
      value = "autoconf, debhelper (>= 9), dpkg-dev (>= 1.15.5), libboost-dev, libboost-filesystem-dev, libcurl4-gnutls-dev | libcurl4-dev, libenet-dev (>= 1.3), libgloox-dev (>= 1.0.9), libicu-dev, libminiupnpc-dev (>= 1.6), libnspr4-dev, libnvtt-dev (>= 2.0.8-1+dfsg-4~), libogg-dev, libopenal-dev, libpng-dev, libsdl2-dev (>= 2.0.2), libvorbis-dev, libwxgtk3.0-dev | libwxgtk2.8-dev, libxcursor-dev, libxml2-dev, pkg-config, python, python3, zlib1g-dev" ),
    ( key = "Architecture",
      value = "amd64 arm64 armhf i386 kfreebsd-amd64 kfreebsd-i386" ),
    ( key = "Standards-Version",
      value = "3.9.8" ),
    (key = "Format", value = "3.0 (quilt)"),
    ( key = "Vcs-Browser",
      value = "https://anonscm.debian.org/viewvc/pkg-games/packages/trunk/0ad/" ),
    ( key = "Vcs-Svn",
      value = "svn://anonscm.debian.org/pkg-games/packages/trunk/0ad/" ),
    ( key = "Homepage",
      value = "http://play0ad.com/" ),
    ( key = "Package-List",
      value = "0ad deb games optional arch=amd64,arm64,armhf,i386,kfreebsd-amd64,kfreebsd-i386" ),
    ( key = "Directory",
      value = "pool/main/0/0ad" ),
    (key = "Priority", value = "source"),
    (key = "Section", value = "games") ] )
( package = "0ad-data",
  version = "0.0.21-1",
  index = "http://deb.debian.org/debian sid/main Sources",
...
```


### License note

The code here is available under the MIT license. However, `libapt-pkg`
is GPLv2, so you must distribute the code under the GPLv2.

Consuming the produced data file does not count as "linking", so you are
free to use whatever license you want for the consumed data.
