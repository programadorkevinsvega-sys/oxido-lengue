# Paquete Debian de Óxido

## Toolchain aislada

El paquete instala la CLI en `/usr/lib/oxido` y un wrapper `/usr/bin/oxido`. Si la toolchain dedicada ya está en `/opt/oxido`, el wrapper la selecciona mediante `RUSTUP_HOME`, `CARGO_HOME` y `PATH` solo para el proceso de Óxido. No modifica el Rust del sistema ni del usuario.

Después de instalar el `.deb`, un administrador puede instalar Rust en el prefijo dedicado con:

```sh
sudo oxido-instalar-toolchain
```

La descarga es explícita. `dpkg` no ejecuta Rustup, `build.rs` ni compiladores desde scripts mantenedores. El instalador usa rustup-init 1.29.1 con SHA-256 fijado para amd64/arm64 y Rust 1.99.1. Ve los checksums en [la documentación de instalación manual de Rustup](https://rust-lang.github.io/rustup/installation/other).

Al eliminar el paquete con `apt remove oxido`, se quitan la CLI y el wrapper. `/opt/oxido/rustup` y `/opt/oxido/cargo` se conservan como datos instalados separadamente; se pueden retirar manualmente cuando ningún otro programa los use.

## Construir el paquete

En un host Debian amd64/arm64 con Rust 1.99.1 o posterior:

```sh
OXIDO_DEB_MAINTAINER="Nombre <correo>" packaging/debian/build-deb.sh
```

El script construye con `cargo build --locked --release -p oxido-cli` y deja el `.deb` en `target/debian/`. Para pruebas solamente admite `--binary`, `--version` y `--output-dir`; todavía no permite compilación cruzada.

## Releases descargables

`.github/workflows/debian-release.yml` construye el paquete en runners Linux nativos amd64 y arm64 en pull requests, ejecuciones manuales y etiquetas `v*`. En PR/manual instala, ejecuta y elimina el `.deb` en cada arquitectura. Solo una etiqueta cuya versión coincida exactamente con `crates/oxido-cli/Cargo.toml` crea o actualiza una GitHub Release con ambos paquetes y `SHA256SUMS`. Pull requests y ejecuciones manuales no publican Releases.

Para instalar una publicación, descarga el `.deb` de tu arquitectura y `SHA256SUMS` de la misma Release, ejecuta `sha256sum --check --ignore-missing SHA256SUMS` e instala con `sudo apt install ./oxido_*_amd64.deb` (o `*_arm64.deb`). Los hashes verifican integridad frente a alteraciones; no son una firma de identidad. Los paquetes no están firmados.

La misma Release incluye los VSIX por plataforma y un paquete `oxido-vscode` Debian amd64/arm64. Este paquete es opcional y separado de la CLI: deja el VSIX en `/usr/share/oxido-vscode/extension.vsix` y el usuario lo instala en su perfil con `oxido-vscode-instalar` o desde la opción **Install from VSIX...** de VS Code. No modifica perfiles durante `apt install`/`apt remove`, no requiere `oxido.deb` y no crea procesos en segundo plano. Lee `../vscode-deb/README.Debian` para el contrato completo.

## Regresiones

```sh
bash packaging/debian/tests/check-rust-toolchain.sh
bash packaging/debian/tests/install-isolated-toolchain.sh
bash packaging/debian/tests/package-lifecycle.sh
bash packaging/vscode-deb/tests/package-lifecycle.sh
bash packaging/debian/tests/prepare-release-assets.sh
```

La regresión de ciclo de vida usa `dpkg` contra una raíz temporal: instala una versión, la actualiza, la elimina y comprueba que los datos bajo `/opt/oxido` sobreviven. Las pruebas del instalador simulan red/checksum. El workflow Debian también instala el paquete real, provisiona Rust 1.99.1 en el prefijo aislado, crea un proyecto con `oxido nuevo`, lo compila con `oxido build` y comprueba tanto el binario como `oxido ejecutar`.
