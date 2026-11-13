#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
builder=$repo_root/packaging/debian/build-deb.sh
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT

make_cli() {
    local path=$1 marker=$2
    cat > "$path" <<EOF
#!/bin/sh
printf '%s\\n' '$marker'
printf 'RUSTUP_HOME=%s\\n' "\${RUSTUP_HOME:-}"
printf 'CARGO_HOME=%s\\n' "\${CARGO_HOME:-}"
printf 'PATH=%s\\n' "\$PATH"
EOF
    chmod 0755 "$path"
}

maintainer='Oxido CI <oxido-ci@example.invalid>'
make_cli "$temp_dir/oxido-v1" 'fixture version 0.1.0'
make_cli "$temp_dir/oxido-v2" 'fixture version 0.2.0'
OXIDO_DEB_MAINTAINER="$maintainer" "$builder" --version 0.1.0 --binary "$temp_dir/oxido-v1" --output-dir "$temp_dir/debs"
OXIDO_DEB_MAINTAINER="$maintainer" "$builder" --version 0.2.0 --binary "$temp_dir/oxido-v2" --output-dir "$temp_dir/debs"
depends=$(dpkg-deb --field "$temp_dir/debs/oxido_0.1.0-1_$(dpkg --print-architecture).deb" Depends)
normalized_depends=${depends//, /,}
for dependency in bash ca-certificates coreutils curl libgcc-s1 libc6 mawk; do
    [[ ",$normalized_depends," == *",$dependency,"* ]]
done
dpkg-deb --control "$temp_dir/debs/oxido_0.1.0-1_$(dpkg --print-architecture).deb" "$temp_dir/control"
[[ $(find "$temp_dir/control" -maxdepth 1 -type f | wc -l) -eq 1 ]]

root=$temp_dir/root
mkdir -p "$root/var/lib/dpkg"
: > "$root/var/lib/dpkg/status"

dpkg --root="$root" --force-not-root --force-script-chrootless --force-depends --install "$temp_dir/debs/oxido_0.1.0-1_$(dpkg --print-architecture).deb"
[[ $(dpkg-query --admindir="$root/var/lib/dpkg" --show --showformat='${Version}' oxido) == 0.1.0-1 ]]
cmp "$temp_dir/oxido-v1" "$root/usr/lib/oxido/oxido"
[[ -x "$root/usr/bin/oxido" && -x "$root/usr/bin/oxido-instalar-toolchain" ]]
[[ -x "$root/usr/lib/oxido/install-isolated-toolchain.sh" && -x "$root/usr/lib/oxido/check-rust-toolchain.sh" ]]
[[ -f "$root/usr/share/doc/oxido/copyright" && -f "$root/usr/share/doc/oxido/README.Debian" ]]
output=$(env -u RUSTUP_HOME -u CARGO_HOME OXIDO_TOOLCHAIN_PREFIX="$root/opt/oxido" "$root/usr/bin/oxido" aliases --markdown)
[[ "$output" == *'fixture version 0.1.0'* && "$output" == *'RUSTUP_HOME='* && "$output" == *'CARGO_HOME='* ]]
mkdir -p "$root/opt/oxido/cargo/bin" "$root/opt/oxido/rustup"
for tool in rustc cargo; do
    printf '#!/bin/sh\nprintf "%%s\\n" preserve-me\n' > "$root/opt/oxido/cargo/bin/$tool"
    chmod 0755 "$root/opt/oxido/cargo/bin/$tool"
done
output=$(env -u RUSTUP_HOME -u CARGO_HOME OXIDO_TOOLCHAIN_PREFIX="$root/opt/oxido" "$root/usr/bin/oxido" aliases --markdown)
[[ "$output" == *"RUSTUP_HOME=$root/opt/oxido/rustup"* && "$output" == *"CARGO_HOME=$root/opt/oxido/cargo"* ]]
[[ "$output" == *"PATH=$root/opt/oxido/cargo/bin:"* ]]
old_rust=$temp_dir/old-rust
mkdir -p "$old_rust"
for tool in bash awk dirname; do
    ln -s "$(command -v "$tool")" "$old_rust/$tool"
done
for tool in rustc cargo; do
    cat > "$old_rust/$tool" <<EOF
#!/bin/sh
printf '$tool 1.85.0\\n'
EOF
    chmod 0755 "$old_rust/$tool"
done
if env -u OXIDO_TOOLCHAIN_PREFIX PATH="$old_rust" "$root/usr/bin/oxido" compilar > "$temp_dir/out" 2>&1; then
    printf 'El wrapper dejó compilar con una toolchain antigua.\n' >&2
    exit 1
fi
grep -Fq '1.99.1 o posterior' "$temp_dir/out"
grep -Fq 'oxido-instalar-toolchain' "$temp_dir/out"
if "$root/usr/bin/oxido-instalar-toolchain" --invalid-option > "$temp_dir/out" 2>&1; then
    printf 'El instalador empaquetado aceptó una opción inválida.\n' >&2
    exit 1
fi
grep -Fq 'Uso:' "$temp_dir/out"

dpkg --root="$root" --force-not-root --force-script-chrootless --force-depends --install "$temp_dir/debs/oxido_0.2.0-1_$(dpkg --print-architecture).deb"
[[ $(dpkg-query --admindir="$root/var/lib/dpkg" --show --showformat='${Version}' oxido) == 0.2.0-1 ]]
cmp "$temp_dir/oxido-v2" "$root/usr/lib/oxido/oxido"
[[ $("$root/opt/oxido/cargo/bin/rustc") == preserve-me ]]

dpkg --root="$root" --force-not-root --force-script-chrootless --remove oxido
[[ ! -e "$root/usr/bin/oxido" && ! -e "$root/usr/bin/oxido-instalar-toolchain" ]]
[[ ! -e "$root/usr/lib/oxido/oxido" && ! -e "$root/usr/lib/oxido/install-isolated-toolchain.sh" ]]
[[ $("$root/opt/oxido/cargo/bin/rustc") == preserve-me ]]
if dpkg-query --admindir="$root/var/lib/dpkg" --show oxido > /dev/null 2>&1; then
    printf 'El paquete sigue registrado después de eliminarlo.\n' >&2
    exit 1
fi
printf 'OK: instalación, actualización y eliminación del .deb.\n'
