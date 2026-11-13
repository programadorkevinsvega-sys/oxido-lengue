#!/usr/bin/env bash
set -euo pipefail

toolchain_version=1.99.1
rustup_version=1.29.1
prefix=${OXIDO_TOOLCHAIN_PREFIX:-/opt/oxido}
case "${1:-}" in
    "") ;;
    --prefix)
        [[ $# -eq 2 && -n "$2" ]] || { printf 'Uso: %s [--prefix DIR]\n' "$0" >&2; exit 2; }
        prefix=$2
        ;;
    *) printf 'Uso: %s [--prefix DIR]\n' "$0" >&2; exit 2 ;;
esac

case "$prefix" in
    /*) ;;
    *) printf 'ERROR: el prefijo debe ser una ruta absoluta.\n' >&2; exit 2 ;;
esac
[[ "$prefix" != / && "$prefix" != *$'\n'* ]] || { printf 'ERROR: prefijo no permitido.\n' >&2; exit 2; }

case "$(uname -m)" in
    x86_64) target=x86_64-unknown-linux-gnu; checksum=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71 ;;
    aarch64) target=aarch64-unknown-linux-gnu; checksum=15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433 ;;
    *) printf 'ERROR: arquitectura no soportada: %s. Compatibles: amd64 y arm64.\n' "$(uname -m)" >&2; exit 1 ;;
esac

for tool in curl sha256sum timeout realpath uname awk dirname mktemp mkdir rm rmdir chmod env; do
    command -v "$tool" >/dev/null 2>&1 || { printf 'ERROR: falta la herramienta requerida: %s.\n' "$tool" >&2; exit 1; }
done

prefix=$(realpath -m -- "$prefix")
rustup_home=$prefix/rustup
cargo_home=$prefix/cargo
if [[ -e "$rustup_home" || -e "$cargo_home" ]]; then
    if [[ -x "$cargo_home/bin/rustc" && -x "$cargo_home/bin/cargo" ]]; then
        checker=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/check-rust-toolchain.sh
        status=0
        report=$(RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" \
            PATH="$cargo_home/bin:$PATH" "$checker" 2>&1) || status=$?
        if (( status == 0 )); then
            [[ -x "$cargo_home/bin/rustfmt" ]] || {
                printf 'ERROR: la toolchain aislada no contiene rustfmt; no se modificó.\n' >&2
                exit 1
            }
            printf 'Toolchain aislada ya compatible en %s. No se modificó.\n' "$prefix"
            exit 0
        fi
        printf '%s\n' "$report" >&2
        printf 'ERROR: la toolchain existente en %s no cumple el requisito y no se reemplazó.\n' "$prefix" >&2
        exit 1
    fi
    printf 'ERROR: existe una instalación parcial en %s; no se modificó.\n' "$prefix" >&2
    exit 1
fi

mkdir -p -- "$prefix" || { printf 'ERROR: no se pudo crear %s (pruebe con permisos adecuados).\n' "$prefix" >&2; exit 1; }
temp_dir=$(mktemp -d)
lock=$prefix/.toolchain-install-lock
if ! mkdir -- "$lock" 2>/dev/null; then
    rm -rf -- "$temp_dir"
    printf 'ERROR: otra instalación está usando %s.\n' "$prefix" >&2
    exit 1
fi
created_rustup=0
created_cargo=0
finish() {
    result=$?
    trap - EXIT
    if ! rm -rf -- "$temp_dir"; then
        printf 'ERROR: no se pudo limpiar el directorio temporal %s.\n' "$temp_dir" >&2
        result=1
    fi
    if ! rmdir -- "$lock"; then
        printf 'ERROR: no se pudo retirar el bloqueo %s.\n' "$lock" >&2
        result=1
    fi
    if (( result != 0 )); then
        if (( created_rustup != 0 )) && ! rm -rf -- "$rustup_home"; then
            printf 'ERROR: no se pudo retirar la instalación parcial %s.\n' "$rustup_home" >&2
        fi
        if (( created_cargo != 0 )) && ! rm -rf -- "$cargo_home"; then
            printf 'ERROR: no se pudo retirar la instalación parcial %s.\n' "$cargo_home" >&2
        fi
    fi
    exit "$result"
}
trap finish EXIT

url=https://static.rust-lang.org/rustup/archive/$rustup_version/$target/rustup-init
if ! curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    --retry 3 --connect-timeout 15 --max-time 300 --max-filesize 50000000 \
    --output "$temp_dir/rustup-init" "$url"; then
    printf 'ERROR: no se pudo descargar rustup-init %s para %s.\n' "$rustup_version" "$target" >&2
    exit 1
fi
printf '%s  %s\n' "$checksum" "$temp_dir/rustup-init" | sha256sum --check --status || {
    printf 'ERROR: el SHA-256 de rustup-init no coincide; se canceló la instalación.\n' >&2
    exit 1
}
chmod 700 -- "$temp_dir/rustup-init"
mkdir -- "$rustup_home"
created_rustup=1
mkdir -- "$cargo_home"
created_cargo=1

if ! timeout --signal=TERM 1800 env RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" \
    "$temp_dir/rustup-init" -y --no-modify-path --profile minimal \
    --default-toolchain "$toolchain_version" --component rustfmt; then
    printf 'ERROR: rustup no pudo instalar Rust %s. Se retiró la instalación parcial aislada.\n' "$toolchain_version" >&2
    exit 1
fi

installed_rustc=$(RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" "$cargo_home/bin/rustc" --version | awk '{print $2}')
installed_cargo=$(RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" "$cargo_home/bin/cargo" --version | awk '{print $2}')
if [[ "$installed_rustc" != "$toolchain_version" || "$installed_cargo" != "$toolchain_version" ]]; then
    printf 'ERROR: verificación posterior fallida (rustc=%s, cargo=%s).\n' "$installed_rustc" "$installed_cargo" >&2
    exit 1
fi
if [[ ! -x "$cargo_home/bin/rustfmt" ]]; then
    printf 'ERROR: rustup terminó sin instalar rustfmt.\n' >&2
    exit 1
fi
checker=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/check-rust-toolchain.sh
status=0
report=$(RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" \
    PATH="$cargo_home/bin:$PATH" "$checker" 2>&1) || status=$?
if (( status != 0 )); then
    printf '%s\n' "$report" >&2
    printf 'ERROR: la verificación posterior de la toolchain falló.\n' >&2
    exit 1
fi
printf 'Rust %s instalado en %s sin modificar la toolchain del usuario.\n' "$toolchain_version" "$prefix"
printf 'Para usar la toolchain aislada en una sesión de shell:\n  export RUSTUP_HOME=%q\n  export CARGO_HOME=%q\n  export PATH=%q:$PATH\n' "$rustup_home" "$cargo_home" "$cargo_home/bin"
