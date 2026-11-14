#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)
builder=$root/packaging/vscode-deb/build-deb.sh
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
architecture=$(dpkg --print-architecture)
target=linux-x64
[[ "$architecture" == arm64 ]] && target=linux-arm64
vsix=$test_dir/oxido-language-support-0.1.0-$target.vsix
payload=$test_dir/vsix-payload
mkdir -p "$payload"
cat > "$payload/extension.vsixmanifest" <<EOF
<PackageManifest><Metadata><Identity Version="0.1.0" TargetPlatform="$target" /></Metadata></PackageManifest>
EOF
(cd "$payload" && zip -q "$vsix" extension.vsixmanifest)
output=$test_dir/packages
maintainer='Oxido CI <oxido-ci@example.invalid>'
OXIDO_VSCODE_DEB_MAINTAINER="$maintainer" bash "$builder" \
    --version 0.1.0 --architecture "$architecture" --vsix "$vsix" \
    --target "$target" --output-dir "$output"
package=$output/oxido-vscode_0.1.0-1_$architecture.deb
[[ $(dpkg-deb --field "$package" Package) == oxido-vscode ]]
[[ $(dpkg-deb --field "$package" Architecture) == "$architecture" ]]
[[ $(dpkg-deb --field "$package" Depends) == bash ]]

stage=$test_dir/root
mkdir -p "$stage/var/lib/dpkg"
mkdir -p "$stage/var/lib/dpkg/info"
profile=$stage/home/fixture/.vscode/extensions/other.extension
mkdir -p "$profile"
printf 'user-owned extension\n' > "$profile/README.txt"
cat > "$stage/var/lib/dpkg/status" <<'EOF'
Package: bash
Status: install ok installed
Version: 5.2
Architecture: all
Maintainer: Oxido CI <oxido-ci@example.invalid>
Description: fixture bash
EOF
: > "$stage/var/lib/dpkg/info/bash.list"
dpkg --root="$stage" --force-not-root --force-script-chrootless \
    --log="$test_dir/dpkg.log" --install "$package"
cmp "$vsix" "$stage/usr/share/oxido-vscode/extension.vsix"
[[ $(cat "$profile/README.txt") == 'user-owned extension' ]]
[[ -x "$stage/usr/bin/oxido-vscode-instalar" ]]
[[ -x "$stage/usr/lib/oxido-vscode/install-extension.sh" ]]

fake_bin=$test_dir/bin
mkdir -p "$fake_bin"
marker=$test_dir/installer-arguments
cat > "$fake_bin/code" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$OXIDO_TEST_MARKER"
EOF
chmod 0755 "$fake_bin/code"
OXIDO_VSCODE_PREFIX="$stage" OXIDO_TEST_MARKER="$marker" PATH="$fake_bin:$PATH" \
    "$stage/usr/bin/oxido-vscode-instalar"
[[ $(sed -n '1p' "$marker") == --install-extension ]]
[[ $(sed -n '2p' "$marker") == "$stage/usr/share/oxido-vscode/extension.vsix" ]]

dpkg --root="$stage" --force-not-root --force-script-chrootless \
    --log="$test_dir/dpkg.log" --remove oxido-vscode
[[ ! -e "$stage/usr/bin/oxido-vscode-instalar" ]]
[[ ! -e "$stage/usr/share/oxido-vscode/extension.vsix" ]]
[[ $(cat "$profile/README.txt") == 'user-owned extension' ]]
printf 'OK: instala el payload, ejecuta el instalador por usuario y elimina el paquete.\n'
