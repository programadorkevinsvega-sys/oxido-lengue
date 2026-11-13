#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
helper=$root/packaging/debian/prepare-release-assets.sh
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
fixture=$test_dir/packages
mkdir -p "$fixture"
for architecture in amd64 arm64; do
    package_root=$test_dir/$architecture
    mkdir -p "$package_root/DEBIAN"
    cat > "$package_root/DEBIAN/control" <<EOF
Package: oxido
Version: 0.1.0-1
Architecture: $architecture
Maintainer: Oxido CI <oxido-ci@example.invalid>
Description: Paquete Debian de prueba
EOF
    printf 'fixture %s\n' "$architecture" > "$package_root/payload"
    dpkg-deb --build --root-owner-group "$package_root" "$fixture/oxido_0.1.0-1_$architecture.deb" >/dev/null
done

bash "$helper" create "$fixture"
bash "$helper" verify "$fixture"
if printf 'tamper\n' >> "$fixture/oxido_0.1.0-1_amd64.deb" && bash "$helper" verify "$fixture"; then
    printf 'ERROR: se aceptó un paquete alterado.\n' >&2
    exit 1
fi
printf 'OK: checksums reproducibles y alteraciones rechazadas.\n'
