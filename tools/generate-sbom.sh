#!/usr/bin/env bash
# Genera SBOM CycloneDX por crate, los renombra por paquete y elimina rutas locales.
# No publica nada: escribe en target/sbom.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out=${1:-"${root}/target/sbom"}

if ! command -v cargo-cyclonedx >/dev/null 2>&1; then
  echo "falta cargo-cyclonedx 0.5.9; instala con: cargo install cargo-cyclonedx --version 0.5.9 --locked" >&2
  exit 1
fi

cd "${root}"
rm -rf "${out}"
mkdir -p "${out}"

cargo cyclonedx --format json --spec-version 1.5 --override-filename oxido-sbom --quiet

count=0
while IFS= read -r file; do
  package=$(basename "$(dirname "${file}")")
  python3 - "${file}" "${root}" "${out}/${package}.cdx.json" <<'PY'
import json
import sys

source, root, destination = sys.argv[1:4]
with open(source, encoding="utf-8") as handle:
    document = json.load(handle)


def sanitize(value):
    if isinstance(value, str):
        return value.replace(root, "/oxido")
    if isinstance(value, list):
        return [sanitize(item) for item in value]
    if isinstance(value, dict):
        return {key: sanitize(item) for key, item in value.items()}
    return value


with open(destination, "w", encoding="utf-8") as handle:
    json.dump(sanitize(document), handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
  rm -f "${file}"
  count=$((count + 1))
done < <(find crates tools -name oxido-sbom.json -type f | sort)

if [ "${count}" -eq 0 ]; then
  echo "cargo-cyclonedx no generó ningún SBOM" >&2
  exit 1
fi

if grep -R -q -F "${root}" "${out}"; then
  echo "el SBOM conserva la ruta absoluta del workspace" >&2
  exit 1
fi

echo "SBOM generados: ${count}"
for file in "${out}"/*.cdx.json; do
  basename "${file}"
done | sort
