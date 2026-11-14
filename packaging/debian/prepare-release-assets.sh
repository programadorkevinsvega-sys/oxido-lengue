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
shopt -s nullglob

collect_assets() {
    local cli_amd64 cli_arm64 vscode_amd64 vscode_arm64 cli_version ext_version release_version
    local expected_vsix=()
    local target asset manifest
    local debs=("$directory"/*.deb)
    local vsix_files=("$directory"/*.vsix)
    local cli_amd64_files=("$directory"/oxido_*_amd64.deb)
    local cli_arm64_files=("$directory"/oxido_*_arm64.deb)
    local vscode_amd64_files=("$directory"/oxido-vscode_*_amd64.deb)
    local vscode_arm64_files=("$directory"/oxido-vscode_*_arm64.deb)
    [[ ${#debs[@]} -eq 4 && ${#vsix_files[@]} -eq 4 ]] || {
        printf 'ERROR: se requieren dos .deb de CLI, dos .deb de VS Code y cuatro VSIX.\n' >&2
        return 1
    }
    [[ ${#cli_amd64_files[@]} -eq 1 && ${#cli_arm64_files[@]} -eq 1 && \
       ${#vscode_amd64_files[@]} -eq 1 && ${#vscode_arm64_files[@]} -eq 1 ]] || {
        printf 'ERROR: falta un paquete Debian esperado o hay nombres ambiguos.\n' >&2
        return 1
    }
    cli_amd64=${cli_amd64_files[0]}
    cli_arm64=${cli_arm64_files[0]}
    vscode_amd64=${vscode_amd64_files[0]}
    vscode_arm64=${vscode_arm64_files[0]}
    cli_version=${cli_amd64##*/oxido_}
    cli_version=${cli_version%_amd64.deb}
    ext_version=${vscode_amd64##*/oxido-vscode_}
    ext_version=${ext_version%_amd64.deb}
    [[ "$cli_version" == "$ext_version" ]] || {
        printf 'ERROR: versiones distintas entre paquetes de CLI y VS Code.\n' >&2
        return 1
    }
    [[ "$cli_version" == *-1 ]] || {
        printf 'ERROR: los paquetes Debian deben usar la revisión -1.\n' >&2
        return 1
    }
    release_version=${cli_version%-1}
    [[ ${cli_arm64##*/oxido_} == "${cli_version}_arm64.deb" && \
       ${vscode_arm64##*/oxido-vscode_} == "${ext_version}_arm64.deb" ]] || {
        printf 'ERROR: las arquitecturas Debian tienen versiones inconsistentes.\n' >&2
        return 1
    }
    for target in linux-x64 linux-arm64 win32-x64 darwin-arm64; do
        expected_vsix+=("$directory/oxido-language-support-${release_version}-${target}.vsix")
    done
    for asset in "${debs[@]}" "${vsix_files[@]}"; do
        [[ -f "$asset" && ! -L "$asset" ]] || {
            printf 'ERROR: asset ausente, no regular o symlink: %s\n' "$asset" >&2
            return 1
        }
    done
    for target in linux-x64 linux-arm64 win32-x64 darwin-arm64; do
        asset=$directory/oxido-language-support-${release_version}-${target}.vsix
        [[ -f "$asset" && ! -L "$asset" ]] || {
            printf 'ERROR: falta el VSIX esperado: %s\n' "$asset" >&2
            return 1
        }
        unzip -tq "$asset" >/dev/null || {
            printf 'ERROR: VSIX corrupto: %s\n' "$asset" >&2
            return 1
        }
        manifest=$(unzip -p "$asset" extension.vsixmanifest) || {
            printf 'ERROR: falta extension.vsixmanifest en %s\n' "$asset" >&2
            return 1
        }
        [[ "$manifest" == *"Version=\"$release_version\""* && \
           "$manifest" == *"TargetPlatform=\"$target\""* ]] || {
            printf 'ERROR: metadatos VSIX no coinciden con versión/target: %s\n' "$asset" >&2
            return 1
        }
    done
    [[ $(dpkg-deb --field "$cli_amd64" Architecture) == amd64 && \
       $(dpkg-deb --field "$cli_arm64" Architecture) == arm64 && \
       $(dpkg-deb --field "$vscode_amd64" Architecture) == amd64 && \
       $(dpkg-deb --field "$vscode_arm64" Architecture) == arm64 && \
       $(dpkg-deb --field "$cli_amd64" Package) == oxido && \
       $(dpkg-deb --field "$cli_arm64" Package) == oxido && \
       $(dpkg-deb --field "$vscode_amd64" Package) == oxido-vscode && \
       $(dpkg-deb --field "$vscode_arm64" Package) == oxido-vscode && \
       $(dpkg-deb --field "$cli_amd64" Version) == "$cli_version" && \
       $(dpkg-deb --field "$cli_arm64" Version) == "$cli_version" && \
       $(dpkg-deb --field "$vscode_amd64" Version) == "$ext_version" && \
       $(dpkg-deb --field "$vscode_arm64" Version) == "$ext_version" ]] || {
        printf 'ERROR: nombre, versión o arquitectura declarada incorrecta en un paquete.\n' >&2
        return 1
    }
    assets=("${debs[@]}" "${expected_vsix[@]}")
}

case "$mode" in
    create)
        collect_assets
        names=()
        for asset in "${assets[@]}"; do names+=("${asset##*/}"); done
        (cd "$directory" && sha256sum -- "${names[@]}" > SHA256SUMS)
        ;;
    verify)
        [[ -f "$directory/SHA256SUMS" && ! -L "$directory/SHA256SUMS" ]] || {
            printf 'ERROR: falta SHA256SUMS regular.\n' >&2
            exit 1
        }
        collect_assets
        expected=$(printf '%s\n' "${assets[@]##*/}" | LC_ALL=C sort)
        listed=$(awk '{ print $2 }' "$directory/SHA256SUMS" | LC_ALL=C sort)
        [[ "$expected" == "$listed" ]] || {
            printf 'ERROR: SHA256SUMS no enumera exactamente todos los assets.\n' >&2
            exit 1
        }
        [[ $(wc -l < "$directory/SHA256SUMS") -eq ${#assets[@]} ]] || {
            printf 'ERROR: SHA256SUMS contiene líneas duplicadas o incompletas.\n' >&2
            exit 1
        }
        (cd "$directory" && sha256sum --check --strict SHA256SUMS)
        ;;
    *) usage; exit 2 ;;
esac
