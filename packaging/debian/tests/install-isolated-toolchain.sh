#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
installer=$root/packaging/debian/install-isolated-toolchain.sh
temp=$(mktemp -d)
trap 'rm -rf "$temp"' EXIT
fakebin=$temp/fakebin
support=$temp/support
mkdir -p "$fakebin" "$support"
for tool in bash awk uname realpath mktemp mkdir rm rmdir chmod env cat cp basename dirname; do
    ln -s "$(command -v "$tool")" "$support/$tool"
done

cat > "$fakebin/curl" <<'EOF'
#!/usr/bin/env bash
output=
while (($# > 0)); do
    if [[ "$1" == --output ]]; then output=$2; shift 2; else shift; fi
done
[[ "${OXIDO_TEST_DOWNLOAD:-ok}" == ok ]] || exit 22
cat "$OXIDO_TEST_FIXTURE" > "$output"
EOF
cat > "$fakebin/sha256sum" <<'EOF'
#!/usr/bin/env bash
[[ "${OXIDO_TEST_CHECKSUM:-ok}" == ok ]]
EOF
cat > "$fakebin/timeout" <<'EOF'
#!/usr/bin/env bash
shift 2
exec "$@"
EOF
chmod +x "$fakebin"/*

fixture=$temp/rustup-init
cat > "$fixture" <<'EOF'
#!/usr/bin/env bash
printf 'install\n' >> "$OXIDO_TEST_CALLS"
mkdir -p "$CARGO_HOME/bin"
for tool in rustc cargo rustfmt; do
    cat > "$CARGO_HOME/bin/$tool" <<'TOOL'
#!/usr/bin/env bash
tool=$(basename "$0")
if [[ "$*" == '--version --verbose' && "$tool" == rustc ]]; then
    printf 'rustc 1.99.1 (isolated-test)\ncommit-hash: abcdef1234567890\n'
elif [[ "$*" == '--version --verbose' && "$tool" == rustfmt ]]; then
    printf 'rustfmt 1.10.0-stable (abcdef123456 test)\n'
else
    printf '%s 1.99.1 (isolated-test)\n' "$tool"
fi
TOOL
    chmod +x "$CARGO_HOME/bin/$tool"
done
[[ "${OXIDO_TEST_INSTALL:-ok}" == ok ]]
EOF

expect() {
    local name=$1 mode=$2 prefix=$3 expected=$4 expected_status=$5 status=0 output
    output=$(OXIDO_TOOLCHAIN_PREFIX="$prefix" OXIDO_TEST_FIXTURE="$fixture" \
        OXIDO_TEST_CALLS="$temp/calls" OXIDO_TEST_DOWNLOAD="${mode%%:*}" \
        OXIDO_TEST_CHECKSUM="${mode#*:}" PATH="$fakebin:$support" \
        "$installer" --prefix "$prefix" 2>&1) || status=$?
    if [[ "$output" != *"$expected"* ]]; then printf '%s\n' "$output" >&2; exit 1; fi
    if [[ "$status" -ne "$expected_status" ]]; then printf '%s\n' "$output" >&2; exit 1; fi
    printf 'OK: %s (exit %s)\n' "$name" "$status"
    printf '%s' "$status" > "$temp/status"
}

prefix=$temp/install
expect 'instalación aislada' 'ok:ok' "$prefix" 'sin modificar la toolchain del usuario' 0
[[ $(cat "$temp/status") -eq 0 ]]
expect 'segunda ejecución idempotente' 'ok:ok' "$prefix" 'ya compatible' 0
[[ $(wc -l < "$temp/calls") -eq 1 ]]

download_prefix=$temp/download-failure
expect 'fallo de descarga' 'fail:ok' "$download_prefix" 'no se pudo descargar rustup-init' 1
[[ $(cat "$temp/status") -ne 0 && ! -e "$download_prefix/rustup" && ! -e "$download_prefix/cargo" ]]

checksum_prefix=$temp/checksum-failure
expect 'checksum incorrecto' 'ok:fail' "$checksum_prefix" 'SHA-256 de rustup-init no coincide' 1
[[ $(cat "$temp/status") -ne 0 && ! -e "$checksum_prefix/rustup" && ! -e "$checksum_prefix/cargo" ]]

partial_prefix=$temp/install-failure
OXIDO_TEST_INSTALL=fail expect 'fallo del instalador limpia el parcial' 'ok:ok' "$partial_prefix" 'Se retiró la instalación parcial aislada' 1
[[ $(cat "$temp/status") -ne 0 && ! -e "$partial_prefix/rustup" && ! -e "$partial_prefix/cargo" ]]

old_prefix=$temp/old
mkdir -p "$old_prefix/rustup" "$old_prefix/cargo/bin"
for tool in rustc cargo; do
    printf '#!/usr/bin/env bash\nprintf "%s 1.85.0\\n"\n' "$tool" > "$old_prefix/cargo/bin/$tool"
    chmod +x "$old_prefix/cargo/bin/$tool"
done
expect 'toolchain aislada antigua se rechaza sin tocarse' 'ok:ok' "$old_prefix" 'rustc detectado (1.85.0)' 1
[[ $(cat "$temp/status") -ne 0 && -x "$old_prefix/cargo/bin/rustc" ]]
[[ $("$old_prefix/cargo/bin/rustc" --version) == 'rustc 1.85.0' ]]
