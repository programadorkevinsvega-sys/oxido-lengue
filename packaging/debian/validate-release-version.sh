#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 || -z "$1" || -z "$2" ]]; then
    printf 'Uso: %s ETIQUETA VERSION_CARGO\n' "$0" >&2
    exit 2
fi

tag=$1
package_version=$2
if [[ "$tag" != v* || "${tag#v}" != "$package_version" ]]; then
    printf 'ERROR: la etiqueta %s debe ser v%s para coincidir con oxido-cli.\n' \
        "$tag" "$package_version" >&2
    exit 1
fi
