#!/bin/sh
set -eu

url=$1
out=$2

T=$(mktemp --tmpdir="$(dirname "$out")")
D=$(mktemp -d --tmpdir=/dev/shm)
trap 'rm -rf '"$T $D" EXIT

mkdir -p "$out"

(
cd "$D"
dget --quiet --allow-unauthenticated --download-only "$url"
dpkg-source --extract --no-check --no-copy --skip-patches ./*.dsc src >/dev/null
#find src \( -name '*.c' -o -name '*.cpp' -o -name '*.C' -o -name '*.h' -o -name '*.java' \) -exec cp {} "$out" \;
cd src/debian
mkdir -p "$out/debian"
cp -ar ./* "$out/debian"
) > "$T"
mv "$T" "$out/debian.log"

