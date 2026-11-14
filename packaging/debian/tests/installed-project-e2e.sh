#!/usr/bin/env bash
set -euo pipefail

toolchain_prefix=${OXIDO_TOOLCHAIN_PREFIX:-/opt/oxido}
rustc=$toolchain_prefix/cargo/bin/rustc
cargo=$toolchain_prefix/cargo/bin/cargo
[[ -x "$rustc" && -x "$cargo" ]] || {
    printf 'ERROR: falta la toolchain aislada en %s.\n' "$toolchain_prefix" >&2
    exit 1
}
rustc_version=$(RUSTUP_HOME="$toolchain_prefix/rustup" "$rustc" --version | awk '{print $2}')
cargo_version=$(RUSTUP_HOME="$toolchain_prefix/rustup" "$cargo" --version | awk '{print $2}')
[[ "$rustc_version" == 1.99.1 && "$cargo_version" == 1.99.1 ]] || {
    printf 'ERROR: versiones aisladas inesperadas (rustc=%s, cargo=%s).\n' \
        "$rustc_version" "$cargo_version" >&2
    exit 1
}

temp_dir=$(mktemp -d)
cleanup() {
    status=$?
    trap - EXIT
    if ! rm -rf -- "$temp_dir"; then
        printf 'ERROR: no se pudo limpiar el directorio temporal %s.\n' "$temp_dir" >&2
        status=1
    fi
    exit "$status"
}
trap cleanup EXIT

project=$temp_dir/demo
env -u RUSTC -u RUSTUP_TOOLCHAIN -u CARGO_HOME -u RUSTUP_HOME oxido nuevo "$project"
source=$project/src/main.ox
[[ -f "$project/oxido.toml" && -f "$source" ]]
env -u RUSTC -u RUSTUP_TOOLCHAIN -u CARGO_HOME -u RUSTUP_HOME oxido build "$source"
generated_project=$project/target/oxido
[[ -f "$generated_project/Cargo.toml" \
    && -f "$generated_project/src/main.ox" \
    && -f "$generated_project/src/main.rs" ]] || {
    printf 'ERROR: `oxido build` no generó el proyecto Cargo y sus fuentes esperados en %s.\n' \
        "$generated_project" >&2
    exit 1
}
binary=$generated_project/target/debug/oxido_generated
[[ -x "$binary" ]] || {
    printf 'ERROR: `oxido build` no generó el binario esperado: %s.\n' "$binary" >&2
    exit 1
}

binary_output=$("$binary")
[[ "$binary_output" == 'Hola, Óxido.' ]] || {
    printf 'ERROR: salida del binario inesperada: %s.\n' "$binary_output" >&2
    exit 1
}
cli_output=$(env -u RUSTC -u RUSTUP_TOOLCHAIN -u CARGO_HOME -u RUSTUP_HOME oxido ejecutar "$source")
[[ "$cli_output" == 'Hola, Óxido.' ]] || {
    printf 'ERROR: `oxido ejecutar` produjo una salida inesperada: %s.\n' "$cli_output" >&2
    exit 1
}

printf 'OK: el .deb instalado creó, compiló y ejecutó un proyecto con Rust %s aislado.\n' \
    "$rustc_version"
