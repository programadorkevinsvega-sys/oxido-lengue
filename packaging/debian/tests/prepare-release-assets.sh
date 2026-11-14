#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
helper=$root/packaging/debian/prepare-release-assets.sh
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
fixture=$test_dir/release
mkdir -p "$fixture"

make_deb() {
    local package=$1 architecture=$2 filename=$3
    local package_root=$test_dir/${filename%%_*}-${architecture}
    mkdir -p "$package_root/DEBIAN"
    cat > "$package_root/DEBIAN/control" <<EOF
Package: $package
Version: 0.1.0-1
Architecture: $architecture
Maintainer: Oxido CI <oxido-ci@example.invalid>
Description: paquete de prueba
EOF
    printf 'fixture %s\n' "$architecture" > "$package_root/payload"
    dpkg-deb --build --root-owner-group "$package_root" "$fixture/$filename" >/dev/null
}

make_deb oxido amd64 oxido_0.1.0-1_amd64.deb
make_deb oxido arm64 oxido_0.1.0-1_arm64.deb
make_deb oxido-vscode amd64 oxido-vscode_0.1.0-1_amd64.deb
make_deb oxido-vscode arm64 oxido-vscode_0.1.0-1_arm64.deb
for target in linux-x64 linux-arm64 win32-x64 darwin-arm64; do
    vsix_root=$test_dir/vsix-$target
    mkdir -p "$vsix_root"
    printf '<PackageManifest><Metadata><Identity Version="0.1.0" TargetPlatform="%s" /></Metadata></PackageManifest>\n' \
        "$target" > "$vsix_root/extension.vsixmanifest"
    (cd "$vsix_root" && zip -q "$fixture/oxido-language-support-0.1.0-$target.vsix" extension.vsixmanifest)
done

bash "$helper" create "$fixture"
bash "$helper" verify "$fixture"
tampered_root=$test_dir/tampered-vsix
mkdir -p "$tampered_root"
printf '<PackageManifest><Metadata><Identity Version="0.1.0" TargetPlatform="linux-x64" /></Metadata><Tampered /></PackageManifest>\n' \
    > "$tampered_root/extension.vsixmanifest"
(cd "$tampered_root" && zip -q "$fixture/oxido-language-support-0.1.0-linux-x64.vsix" extension.vsixmanifest)
if bash "$helper" verify "$fixture" > "$test_dir/tampered.log" 2>&1; then
    printf 'ERROR: se aceptó un asset alterado.\n' >&2
    exit 1
fi
grep -Fq 'FAILED' "$test_dir/tampered.log"
rm "$fixture/oxido-language-support-0.1.0-darwin-arm64.vsix"
if bash "$helper" create "$fixture" > "$test_dir/incomplete.log" 2>&1; then
    printf 'ERROR: se aceptó una release incompleta.\n' >&2
    exit 1
fi
grep -Fq 'se requieren' "$test_dir/incomplete.log"
printf 'OK: checksums cubren CLI, extensión y VSIX; alteraciones/ausencias se rechazan.\n'
