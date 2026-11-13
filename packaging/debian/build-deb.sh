#!/usr/bin/env bash
set -euo pipefail
umask 022

source_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
version=
architecture=$(dpkg --print-architecture)
output_dir=$source_root/target/debian
binary=

usage() {
    printf 'Uso: OXIDO_DEB_MAINTAINER="Nombre <correo>" %s [--version VERSION] [--architecture ARCH] [--output-dir DIR] [--binary PATH]\n' "$0" >&2
}

while (($# > 0)); do
    case "$1" in
        --version|--architecture|--output-dir|--binary)
            [[ $# -ge 2 && -n "$2" ]] || { usage; exit 2; }
            case "$1" in
                --version) version=$2 ;;
                --architecture) architecture=$2 ;;
                --output-dir) output_dir=$2 ;;
                --binary) binary=$2 ;;
            esac
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) usage; printf 'ERROR: opción desconocida: %s\n' "$1" >&2; exit 2 ;;
    esac
done

maintainer=${OXIDO_DEB_MAINTAINER:-}
if [[ -z "$maintainer" || "$maintainer" =~ [[:cntrl:]] || "$maintainer" != *'<'*'>'* ]]; then
    printf 'ERROR: defina OXIDO_DEB_MAINTAINER como "Nombre <correo>" sin caracteres de control.\n' >&2
    exit 2
fi
if [[ -z "$version" ]]; then
    version=$(awk '/^\[package\]$/ { package=1; next } /^\[/ { package=0 } package && /^version = / { gsub(/"/, "", $3); print $3; exit }' "$source_root/crates/oxido-cli/Cargo.toml")
fi
[[ "$version" =~ ^[0-9][A-Za-z0-9.+:~.-]*$ ]] || { printf 'ERROR: versión Debian no válida: %s\n' "$version" >&2; exit 2; }
[[ "$architecture" == amd64 || "$architecture" == arm64 ]] || {
    printf 'ERROR: arquitectura Debian no soportada: %s (compatibles: amd64, arm64).\n' "$architecture" >&2
    exit 1
}
[[ "$architecture" == "$(dpkg --print-architecture)" ]] || {
    printf 'ERROR: la compilación cruzada no está implementada; arquitectura de este host: %s.\n' "$(dpkg --print-architecture)" >&2
    exit 1
}

if [[ -z "$binary" ]]; then
    cargo build --locked --release --manifest-path "$source_root/Cargo.toml" -p oxido-cli
    binary=$source_root/target/release/oxido
fi
[[ -f "$binary" && -x "$binary" ]] || { printf 'ERROR: binario Óxido ausente o no ejecutable: %s\n' "$binary" >&2; exit 1; }

output_dir=$(mkdir -p -- "$output_dir" && cd "$output_dir" && pwd -P)
package_path=$output_dir/oxido_${version}-1_${architecture}.deb
[[ ! -e "$package_path" ]] || { printf 'ERROR: el paquete ya existe: %s\n' "$package_path" >&2; exit 1; }
work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT
package_root=$work_dir/root
mkdir -p "$package_root/DEBIAN" "$package_root/usr/bin" "$package_root/usr/lib/oxido" "$package_root/usr/share/doc/oxido"
install -m 0755 "$binary" "$package_root/usr/lib/oxido/oxido"
install -m 0755 "$source_root/packaging/debian/payload/usr/bin/oxido" "$package_root/usr/bin/oxido"
install -m 0755 "$source_root/packaging/debian/payload/usr/bin/oxido-instalar-toolchain" "$package_root/usr/bin/oxido-instalar-toolchain"
install -m 0755 "$source_root/packaging/debian/install-isolated-toolchain.sh" "$package_root/usr/lib/oxido/install-isolated-toolchain.sh"
install -m 0755 "$source_root/packaging/debian/check-rust-toolchain.sh" "$package_root/usr/lib/oxido/check-rust-toolchain.sh"
install -m 0644 "$source_root/LICENSE-MIT" "$package_root/usr/share/doc/oxido/LICENSE-MIT"
install -m 0644 "$source_root/LICENSE-APACHE" "$package_root/usr/share/doc/oxido/LICENSE-APACHE"
install -m 0644 "$source_root/packaging/debian/copyright" "$package_root/usr/share/doc/oxido/copyright"
install -m 0644 "$source_root/packaging/debian/README.Debian" "$package_root/usr/share/doc/oxido/README.Debian"
installed_size=$(du -sk "$package_root" | awk '{print $1}')
cat > "$package_root/DEBIAN/control" <<EOF
Package: oxido
Version: ${version}-1
Section: devel
Priority: optional
Architecture: $architecture
Installed-Size: $installed_size
Depends: bash, ca-certificates, coreutils, curl, libgcc-s1, libc6, mawk
Maintainer: $maintainer
Description: lenguaje de programación en español que genera Rust
 CLI, compilador y herramienta para instalar una toolchain Rust aislada.
EOF
chmod 0755 "$package_root/DEBIAN"
dpkg-deb --build --root-owner-group "$package_root" "$package_path"
printf 'Paquete creado: %s\n' "$package_path"
