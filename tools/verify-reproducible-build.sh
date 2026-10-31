#!/usr/bin/env bash
# Verifica que dos compilaciones release del mismo commit producen bytes idénticos.
# No publica nada: solo compila en directorios temporales y compara SHA-256.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
package=${1:-oxido-cli}
binary=${2:-oxido}
profile=${3:-release}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/oxido-repro.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

export SOURCE_DATE_EPOCH=0
export CARGO_INCREMENTAL=0
export RUSTFLAGS="--remap-path-prefix=${root}=/oxido"

for attempt in a b; do
  CARGO_TARGET_DIR="${tmp}/${attempt}" \
    cargo build --quiet --locked --profile "${profile}" -p "${package}"
done

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "no se encontró sha256sum ni shasum" >&2
    exit 1
  fi
}

first="${tmp}/a/${profile}/${binary}"
second="${tmp}/b/${profile}/${binary}"

if [ ! -f "${first}" ] || [ ! -f "${second}" ]; then
  echo "no se encontró el binario ${binary} en el perfil ${profile}" >&2
  exit 1
fi

first_hash=$(hash_file "${first}")
second_hash=$(hash_file "${second}")

if [ "${first_hash}" != "${second_hash}" ]; then
  echo "build no reproducible para ${package}/${binary}" >&2
  echo "  compilación A: ${first_hash}" >&2
  echo "  compilación B: ${second_hash}" >&2
  exit 1
fi

echo "build reproducible: ${package}/${binary} (${profile})"
echo "sha256: ${first_hash}"
