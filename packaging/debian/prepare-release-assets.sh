#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Uso: %s create|verify DIR\n' "$0" >&2
}

[[ $# -eq 2 ]] || { usage; exit 2; }
mode=$1
directory=$2
[[ -d "$directory" ]] || { printf 'ERROR: no existe el directorio: %s\n' "$directory" >&2; exit 2; }
directory=$(cd "$directory" && pwd -P)

case "$mode" in
    create)
        shopt -s nullglob
        amd64=("$directory"/oxido_*_amd64.deb)
        arm64=("$directory"/oxido_*_arm64.deb)
        [[ ${#amd64[@]} -eq 1 && ${#arm64[@]} -eq 1 ]] || {
            printf 'ERROR: se requiere exactamente un .deb amd64 y uno arm64.\n' >&2
            exit 1
        }
        for package in "${amd64[0]}" "${arm64[0]}"; do
            [[ -f "$package" && ! -L "$package" ]] || {
                printf 'ERROR: paquete ausente o enlace simbólico: %s\n' "$package" >&2
                exit 1
            }
        done
        amd64_name=${amd64[0]##*/}
        arm64_name=${arm64[0]##*/}
        amd64_version=${amd64_name#oxido_}
        amd64_version=${amd64_version%_amd64.deb}
        arm64_version=${arm64_name#oxido_}
        arm64_version=${arm64_version%_arm64.deb}
        [[ "$amd64_version" == "$arm64_version" ]] || {
            printf 'ERROR: versiones Debian distintas entre arquitecturas.\n' >&2
            exit 1
        }
        [[ $(dpkg-deb --field "${amd64[0]}" Architecture) == amd64 ]] || exit 1
        [[ $(dpkg-deb --field "${arm64[0]}" Architecture) == arm64 ]] || exit 1
        (cd "$directory" && sha256sum -- "$amd64_name" "$arm64_name" > SHA256SUMS)
        ;;
    verify)
        [[ -f "$directory/SHA256SUMS" && ! -L "$directory/SHA256SUMS" ]] || {
            printf 'ERROR: falta SHA256SUMS regular.\n' >&2
            exit 1
        }
        shopt -s nullglob
        amd64=("$directory"/oxido_*_amd64.deb)
        arm64=("$directory"/oxido_*_arm64.deb)
        line_count=$(wc -l < "$directory/SHA256SUMS")
        [[ ${#amd64[@]} -eq 1 && ${#arm64[@]} -eq 1 && "$line_count" -eq 2 ]] || {
            printf 'ERROR: SHA256SUMS debe contener exactamente dos paquetes.\n' >&2
            exit 1
        }
        expected_names=$(printf '%s\n' "${amd64[0]##*/}" "${arm64[0]##*/}" | LC_ALL=C sort)
        listed_names=$(awk '{ print $2 }' "$directory/SHA256SUMS" | LC_ALL=C sort)
        [[ "$expected_names" == "$listed_names" ]] || {
            printf 'ERROR: SHA256SUMS no enumera los paquetes amd64 y arm64 esperados.\n' >&2
            exit 1
        }
        (cd "$directory" && sha256sum --check --strict SHA256SUMS)
        ;;
    *) usage; exit 2 ;;
esac
