#!/usr/bin/env bash
set -euo pipefail

minimum_version=1.99.1
failed=0

version_of() {
    "$1" --version | awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^[0-9]+\.[0-9]+\.[0-9]+$/) { print $i; exit } }'
}

commit_hash_of() {
    local version_output
    version_output=$("$1" --version --verbose) || return $?
    awk '
        $1 == "commit-hash:" { print $2; exit }
        NR == 1 && $1 == "rustfmt" { gsub(/[()]/, "", $3); print $3; exit }
    ' <<< "$version_output"
}

at_least_minimum() {
    awk -v actual="$1" -v minimum="$minimum_version" 'BEGIN {
        split(actual, a, "\\."); split(minimum, m, "\\.");
        for (i = 1; i <= 3; i++) {
            if ((a[i] + 0) > (m[i] + 0)) exit 0;
            if ((a[i] + 0) < (m[i] + 0)) exit 1;
        }
        exit 0;
    }'
}

rustc_path=$(command -v rustc || true)
cargo_path=$(command -v cargo || true)

if [[ -z "$rustc_path" && -z "$cargo_path" ]]; then
    printf 'Rust no está instalado; se requiere al menos %s.\n' "$minimum_version"
    printf 'Este preflight no instala ni modifica toolchains; la provisión aislada se implementará en otra subfase.\n'
    exit 0
fi

if [[ -z "$rustc_path" || -z "$cargo_path" ]]; then
    printf 'ERROR: rustc y cargo deben estar disponibles juntos.\n' >&2
    failed=1
fi

rustc_version=
cargo_version=
if [[ -n "$rustc_path" ]]; then
    rustc_version=$(version_of rustc)
    if [[ -z "$rustc_version" ]] || ! at_least_minimum "$rustc_version"; then
        printf 'ERROR: rustc detectado (%s) es anterior a %s o no es estable.\n' "${rustc_version:-versión desconocida}" "$minimum_version" >&2
        failed=1
    else
        printf 'rustc %s: compatible (mínimo %s).\n' "$rustc_version" "$minimum_version"
    fi
fi

if [[ -n "$cargo_path" ]]; then
    cargo_version=$(version_of cargo)
    if [[ -z "$cargo_version" ]] || ! at_least_minimum "$cargo_version"; then
        printf 'ERROR: cargo detectado (%s) es anterior a %s o no es estable.\n' "${cargo_version:-versión desconocida}" "$minimum_version" >&2
        failed=1
    else
        printf 'cargo %s: compatible (mínimo %s).\n' "$cargo_version" "$minimum_version"
    fi
fi

if [[ -n "$rustc_path" && -n "$cargo_path" ]]; then
    rustc_commit=
    if ! rustc_commit=$(commit_hash_of rustc); then
        printf 'ERROR: no se pudo consultar `rustc --version --verbose`.\n' >&2
        failed=1
    elif [[ -z "$rustc_commit" ]]; then
        printf 'ERROR: no se pudo identificar el commit de rustc.\n' >&2
        failed=1
    elif command -v rustfmt >/dev/null 2>&1; then
        rustfmt_commit=
        if ! rustfmt_commit=$(commit_hash_of rustfmt); then
            printf 'ERROR: no se pudo consultar `rustfmt --version --verbose`.\n' >&2
            failed=1
        elif [[ -z "$rustfmt_commit" || ${#rustfmt_commit} -lt 7 || "$rustc_commit" != "$rustfmt_commit"* ]]; then
            printf 'ERROR: rustfmt no coincide con rustc (rustc=%s, rustfmt=%s).\n' \
                "$rustc_commit" "${rustfmt_commit:-desconocido}" >&2
            failed=1
        else
            printf 'rustfmt coincide con el commit de rustc (%s).\n' "$rustc_commit"
        fi
    else
        printf 'Aviso: rustfmt no está instalado; no se modificó la toolchain existente.\n'
    fi
fi

if (( failed != 0 )); then
    printf 'Requisito: Rust estable %s o posterior. No se instaló, actualizó ni desinstaló ningún componente.\n' "$minimum_version" >&2
    exit 1
fi

printf 'Preflight de toolchain correcto.\n'
