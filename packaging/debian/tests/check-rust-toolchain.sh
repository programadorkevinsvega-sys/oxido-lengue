#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
checker="$repo_root/packaging/debian/check-rust-toolchain.sh"
temp_root=$(mktemp -d)
trap 'rm -rf "$temp_root"' EXIT
support_path="$temp_root/support"
mkdir -p "$support_path"
ln -s "$(command -v bash)" "$support_path/bash"
ln -s "$(command -v awk)" "$support_path/awk"

fail() {
    printf 'FALLO: %s\n' "$1" >&2
    exit 1
}

make_tool() {
    local directory=$1 tool=$2 version=$3 commit=$4
    mkdir -p "$directory"
    cat > "$directory/$tool" <<EOF
#!/usr/bin/env bash
if [[ "\$*" == "--version --verbose" ]]; then
    if [[ '$tool' == rustfmt ]]; then
        printf '%s %s (%s test)\\n' '$tool' '$version' '$commit'
    else
        printf '%s %s (test)\\n' '$tool' '$version'
        printf 'commit-hash: %s\\n' '$commit'
    fi
else
    printf '%s %s\\n' '$tool' '$version'
fi
EOF
    chmod +x "$directory/$tool"
}

run_case() {
    local name=$1 expected_status=$2 expected_text=$3 path=$4
    local output status=0
    output=$(PATH="$path:$support_path" "$checker" 2>&1) || status=$?
    if [[ "$status" -ne "$expected_status" ]]; then
        printf '%s\n' "$output" >&2
        fail "$name: estado $status, esperado $expected_status"
    fi
    if [[ "$output" != *"$expected_text"* ]]; then
        printf '%s\n' "$output" >&2
        fail "$name: falta texto esperado: $expected_text"
    fi
    printf 'OK: %s\n' "$name"
}

valid="$temp_root/valid"
make_tool "$valid" rustc 1.99.1 abc1234567
make_tool "$valid" cargo 1.99.1 cargo123
make_tool "$valid" rustfmt 1.10.0-stable abc1234567
run_case 'toolchain compatible y rustfmt alineado' 0 'Preflight de toolchain correcto' "$valid"

old="$temp_root/old"
make_tool "$old" rustc 1.85.0 old1234567
make_tool "$old" cargo 1.99.1 cargo123
make_tool "$old" rustfmt 1.10.0-stable old1234567
run_case 'rustc anterior se rechaza sin modificarlo' 1 'anterior a 1.99.1' "$old"

mismatch="$temp_root/mismatch"
make_tool "$mismatch" rustc 1.99.1 abc1234567
make_tool "$mismatch" cargo 1.99.1 cargo123
make_tool "$mismatch" rustfmt 1.10.0-stable other4567890
run_case 'rustfmt de otro toolchain se rechaza' 1 'rustfmt no coincide' "$mismatch"

missing="$temp_root/missing"
mkdir -p "$missing"
run_case 'ausencia de Rust se informa sin instalarlo' 0 'Rust no está instalado' "$missing"
