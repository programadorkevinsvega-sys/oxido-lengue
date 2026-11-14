#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
validator=$root/packaging/debian/validate-release-version.sh
bash "$validator" v0.1.0 0.1.0
bash "$validator" v0.1.0-rc.1 0.1.0-rc.1
for invalid_tag in 0.1.0 v0.2.0 v0.1.0+meta; do
    if bash "$validator" "$invalid_tag" 0.1.0 >/dev/null 2>&1; then
        printf 'ERROR: se aceptó una etiqueta o versión incoherente: %s\n' "$invalid_tag" >&2
        exit 1
    fi
done
printf 'OK: etiqueta y versión de Cargo deben coincidir exactamente.\n'
