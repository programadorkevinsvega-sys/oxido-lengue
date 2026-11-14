#!/usr/bin/env bash
set -euo pipefail
umask 022

source_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
version=
architecture=$(dpkg --print-architecture)
output_dir=$source_root/target/debian-vscode
vsix=
target=
maintainer=${OXIDO_VSCODE_DEB_MAINTAINER:-}

usage() {
    printf 'Uso: OXIDO_VSCODE_DEB_MAINTAINER="Nombre <correo>" %s --vsix FILE --target TARGET [--version VERSION] [--architecture ARCH] [--output-dir DIR]\n' "$0" >&2
}

while (($# > 0)); do
    case "$1" in
        --version|--architecture|--output-dir|--vsix|--target)
            [[ $# -ge 2 && -n "$2" ]] || { usage; exit 2; }
            case "$1" in
                --version) version=$2 ;;
                --architecture) architecture=$2 ;;
                --output-dir) output_dir=$2 ;;
                --vsix) vsix=$2 ;;
                --target) target=$2 ;;
            esac
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) usage; printf 'ERROR: opción desconocida: %s\n' "$1" >&2; exit 2 ;;
    esac
done

[[ -n "$maintainer" && ! "$maintainer" =~ [[:cntrl:]] && "$maintainer" == *'<'*'>'* ]] || {
    printf 'ERROR: defina OXIDO_VSCODE_DEB_MAINTAINER como "Nombre <correo>".\n' >&2
    exit 2
}
[[ -f "$vsix" && ! -L "$vsix" ]] || { printf 'ERROR: VSIX ausente o no regular: %s\n' "$vsix" >&2; exit 2; }
[[ "$version" =~ ^[0-9][A-Za-z0-9.+:~.-]*$ ]] || { printf 'ERROR: versión no válida: %s\n' "$version" >&2; exit 2; }
[[ "$architecture" == amd64 || "$architecture" == arm64 ]] || {
    printf 'ERROR: arquitectura Debian no soportada: %s\n' "$architecture" >&2
    exit 1
}
expected_target=linux-x64
[[ "$architecture" == arm64 ]] && expected_target=linux-arm64
[[ "$target" == "$expected_target" && "$(basename "$vsix")" == "oxido-language-support-${version}-${target}.vsix" ]] || {
    printf 'ERROR: el VSIX no corresponde a la versión y arquitectura Debian.\n' >&2
    exit 2
}
command -v unzip >/dev/null 2>&1 || {
    printf 'ERROR: se requiere unzip para validar el VSIX.\n' >&2
    exit 2
}
unzip -tq "$vsix" >/dev/null || { printf 'ERROR: VSIX corrupto: %s\n' "$vsix" >&2; exit 2; }
manifest=$(unzip -p "$vsix" extension.vsixmanifest) || {
    printf 'ERROR: el VSIX no contiene extension.vsixmanifest.\n' >&2
    exit 2
}
grep -Fq "Version=\"$version\"" <<< "$manifest" || {
    printf 'ERROR: la versión del manifiesto VSIX no coincide.\n' >&2
    exit 2
}
grep -Fq "TargetPlatform=\"$target\"" <<< "$manifest" || {
    printf 'ERROR: la plataforma del manifiesto VSIX no coincide.\n' >&2
    exit 2
}
[[ "$architecture" == "$(dpkg --print-architecture)" ]] || {
    printf 'ERROR: la compilación cruzada no está implementada para Debian.\n' >&2
    exit 1
}

output_dir=$(mkdir -p -- "$output_dir" && cd "$output_dir" && pwd -P)
package_path=$output_dir/oxido-vscode_${version}-1_${architecture}.deb
[[ ! -e "$package_path" ]] || { printf 'ERROR: el paquete ya existe: %s\n' "$package_path" >&2; exit 1; }
work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT
package_root=$work_dir/root
mkdir -p "$package_root/DEBIAN" "$package_root/usr/bin" \
    "$package_root/usr/share/oxido-vscode" "$package_root/usr/lib/oxido-vscode" \
    "$package_root/usr/share/doc/oxido-vscode"
install -m 0644 "$vsix" "$package_root/usr/share/oxido-vscode/extension.vsix"
install -m 0755 "$source_root/packaging/vscode-deb/payload/usr/bin/oxido-vscode-instalar" \
    "$package_root/usr/bin/oxido-vscode-instalar"
install -m 0755 "$source_root/packaging/vscode-deb/install-extension.sh" \
    "$package_root/usr/lib/oxido-vscode/install-extension.sh"
install -m 0644 "$source_root/LICENSE-MIT" "$package_root/usr/share/doc/oxido-vscode/LICENSE-MIT"
install -m 0644 "$source_root/LICENSE-APACHE" "$package_root/usr/share/doc/oxido-vscode/LICENSE-APACHE"
install -m 0644 "$source_root/packaging/vscode-deb/README.Debian" \
    "$package_root/usr/share/doc/oxido-vscode/README.Debian"
installed_size=$(du -sk "$package_root" | awk '{print $1}')
cat > "$package_root/DEBIAN/control" <<EOF
Package: oxido-vscode
Version: ${version}-1
Section: devel
Priority: optional
Architecture: $architecture
Installed-Size: $installed_size
Depends: bash
Maintainer: $maintainer
Description: extensión de VS Code para el lenguaje Óxido
 VSIX autocontenido con el servidor LSP de Óxido para Linux $architecture.
EOF
chmod 0755 "$package_root/DEBIAN"
dpkg-deb --build --root-owner-group "$package_root" "$package_path"
printf 'Paquete creado: %s\n' "$package_path"
