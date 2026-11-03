#!/usr/bin/env bash
# Runs the tokprof of a release archive over the log sources' fixture logs, or compares the share
# files that such runs wrote.
#
#   ci/smoke.sh <archive.tar.gz> <out.json>   unpack it, run --version, write the share file
#   ci/smoke.sh --compare <file>...           check that the files hold the same numbers
#
# The run reads nothing of the machine it runs on: HOME, XDG_CONFIG_HOME and TOKPROF_DIR point
# into a temporary folder, so no price file or profile directory is read, and TZ=UTC keeps the
# window's days the same on every runner.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)

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
  echo "usage: ci/smoke.sh <archive.tar.gz> <out.json> | ci/smoke.sh --compare <file>..." >&2
  exit 2
fi
archive=$1
out=$2

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -xzf "$archive" -C "$work"
if [ ! -x "$work/tokprof" ]; then
  echo "smoke: $archive holds no tokprof at its top level" >&2
  exit 1
fi

"$work/tokprof" --version
mkdir -p "$(dirname "$out")"
HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" TOKPROF_DIR="$work/profiles" TZ=UTC \
  CLAUDE_CONFIG_DIR="$root/sources/claude-code/tests/fixtures" CODEX_HOME="$root/sources/codex/tests/fixtures" \
  "$work/tokprof" share -o "$out"
