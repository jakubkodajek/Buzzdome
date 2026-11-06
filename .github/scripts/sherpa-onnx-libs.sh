#!/usr/bin/env bash
# Fetches the prebuilt sherpa-onnx libraries that crates/voice links into
# the app, checks each against the SHA-256 written here, and points the
# sherpa-onnx-sys build at them (SHERPA_ONNX_ARCHIVE_DIR), so the build
# never links a download nobody checked (docs/releasing.md).
#
# Usage: sherpa-onnx-libs.sh <repo root>. Picks this runner's archives
# (both Mac ones, for the universal build) and, on GitHub Actions, writes
# SHERPA_ONNX_ARCHIVE_DIR to $GITHUB_ENV.
set -euo pipefail

root="${1:-.}"

# The archives, by file name. A new sherpa-onnx-sys version has new names,
# so this script fails until their hashes are written here.
# (A function, not an associative array: macOS runs bash 3.)
pinned() {
  case "$1" in
    sherpa-onnx-v2.0.1-linux-x64-static-lib.tar.bz2) echo 55cfadf65ba50d0afc1a802b7a337942819e51c94a5b5a384def1978f7c70871 ;;
    sherpa-onnx-v2.0.1-osx-arm64-static-lib.tar.bz2) echo d7a9b14f337958e7373c8c650f046df33d210bbaff2f33cb8322dda663ee5e1c ;;
    sherpa-onnx-v2.0.1-osx-x64-static-lib.tar.bz2) echo 6a3cb3c14104d3a02e7b2a6cd5621f590b14892cce7d0bc0935f7655ef518938 ;;
    sherpa-onnx-v2.0.1-win-x64-static-MT-Release-lib.tar.bz2) echo 23e7b986b197eda0bbc47ff3d7d5c87afd6ef2b9077214ba26b0f1ec52ef0e66 ;;
  esac
}

# The version Cargo.lock holds the build to.
version=$(awk '/^name = "sherpa-onnx-sys"$/ { getline; gsub(/"/, "", $3); print $3 }' "$root/Cargo.lock")
if [ -z "$version" ]; then
  echo "::error::sherpa-onnx-sys is not in Cargo.lock"
  exit 1
fi

case "$(uname -s)" in
  Linux) platforms=(linux-x64-static) ;;
  Darwin) platforms=(osx-arm64-static osx-x64-static) ;;
  MINGW* | MSYS* | CYGWIN*) platforms=(win-x64-static-MT-Release) ;;
  *) echo "::error::No sherpa-onnx libraries for $(uname -s)"; exit 1 ;;
esac

# Read from standard input: given a path, Git Bash's sha256sum on Windows
# starts its line with a backslash.
sha256() {
  if command -v sha256sum > /dev/null; then sha256sum < "$1" | cut -d' ' -f1; else shasum -a 256 < "$1" | cut -d' ' -f1; fi
}

# Named for the run on CI: the build script runs again only when this
# folder's name changes (rerun-if-env-changed), and it must run again to
# unpack the libraries removed below.
dir="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/sherpa-onnx-archives${GITHUB_RUN_ID:+-$GITHUB_RUN_ID-${GITHUB_RUN_ATTEMPT:-1}}"
mkdir -p "$dir"
for platform in "${platforms[@]}"; do
  name="sherpa-onnx-v$version-$platform-lib.tar.bz2"
  expected=$(pinned "$name")
  if [ -z "$expected" ]; then
    echo "::error::No SHA-256 for $name: write it into .github/scripts/sherpa-onnx-libs.sh"
    exit 1
  fi
  if [ ! -f "$dir/$name" ] || [ "$(sha256 "$dir/$name")" != "$expected" ]; then
    curl -fsSL --retry 3 -o "$dir/$name" "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version/$name"
  fi
  actual=$(sha256 "$dir/$name")
  if [ "$actual" != "$expected" ]; then
    echo "::error::$name has SHA-256 $actual, not $expected"
    exit 1
  fi
done

# The build script links whatever it finds unpacked in the target folder and
# never looks at its archive again; a cache may bring one back partial or
# changed. So none is kept: it unpacks the checked archives afresh.
rm -rf "$root/target/sherpa-onnx-prebuilt"

echo "sherpa-onnx $version libraries checked in $dir"
if [ -n "${GITHUB_ENV:-}" ]; then
  echo "SHERPA_ONNX_ARCHIVE_DIR=$dir" >> "$GITHUB_ENV"
fi
