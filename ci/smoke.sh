#!/usr/bin/env bash
# Runs the tokprof of a release archive over the log sources' fixture logs, or compares the share
# files that such runs wrote.
#
#   ci/smoke.sh <archive.tar.gz|archive.zip> <out.json>   unpack it, run --version, write the share file
#   ci/smoke.sh --compare <file>...                        check that the files hold the same numbers
#
# The run reads nothing of the machine it runs on: HOME, USERPROFILE (where Windows takes the home
# directory from), XDG_CONFIG_HOME and TOKPROF_DIR point into a temporary folder, so no price file
# or profile directory is read. TZ=UTC keeps the window's days the same on every Unix runner; the
# Windows runners run in UTC.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)

# A path as the binary takes it. Under Git Bash, a native Windows program gets `D:/a/...` and not
# `/d/a/...`; elsewhere the path stays as it is.
native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi
}

if [ "${1:-}" = --compare ]; then
  shift
  if [ $# -lt 2 ]; then
    echo "smoke: --compare needs at least two files" >&2
    exit 2
  fi
  # The fields that name the platform differ by design.
  normalize() { jq -S 'del(.platform)' "$1"; }
  first=$1
  want=$(normalize "$first")
  for file in "${@:2}"; do
    if [ "$(normalize "$file")" != "$want" ]; then
      echo "smoke: $file holds other numbers than $first:" >&2
      diff <(printf '%s\n' "$want") <(normalize "$file") | head -n 20 >&2 || true
      exit 1
    fi
  done
  echo "smoke: the $# files hold the same numbers"
  exit 0
fi

if [ $# -ne 2 ]; then
  echo "usage: ci/smoke.sh <archive.tar.gz|archive.zip> <out.json> | ci/smoke.sh --compare <file>..." >&2
  exit 2
fi
archive=$1
out=$2

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
case "$archive" in
  *.zip)
    # 7-Zip is on the Windows runners, which have no unzip; a Mac has unzip.
    if command -v 7z >/dev/null 2>&1; then
      7z x -y -bso0 -bsp0 -o"$(native "$work")" "$(native "$archive")"
    else
      unzip -q "$archive" -d "$work"
    fi
    ;;
  *) tar -xzf "$archive" -C "$work" ;;
esac
bin="$work/tokprof"
if [ -f "$work/tokprof.exe" ]; then bin="$work/tokprof.exe"; fi
if [ ! -x "$bin" ]; then
  echo "smoke: $archive holds no tokprof or tokprof.exe at its top level" >&2
  exit 1
fi

"$bin" --version
mkdir -p "$(dirname "$out")"
HOME="$(native "$work/home")" USERPROFILE="$(native "$work/home")" XDG_CONFIG_HOME="$(native "$work/home/.config")" \
  TOKPROF_DIR="$(native "$work/profiles")" TZ=UTC \
  CLAUDE_CONFIG_DIR="$(native "$root/sources/claude-code/tests/fixtures")" CODEX_HOME="$(native "$root/sources/codex/tests/fixtures")" \
  "$bin" share -o "$(native "$out")"
