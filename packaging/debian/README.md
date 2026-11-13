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

El script construye con `cargo build --locked --release -p oxido-cli` y deja el `.deb` en `target/debian/`. Para pruebas solamente admite `--binary`, `--version` y `--output-dir`; todavía no permite compilación cruzada ni publica paquetes.

## Regresiones

```sh
bash packaging/debian/tests/check-rust-toolchain.sh
bash packaging/debian/tests/install-isolated-toolchain.sh
bash packaging/debian/tests/package-lifecycle.sh
```

La regresión de ciclo de vida usa `dpkg` contra una raíz temporal: instala una versión, la actualiza, la elimina y comprueba que los datos bajo `/opt/oxido` sobreviven. Las pruebas del instalador simulan red/checksum; la instalación real aislada de Rust también fue verificada localmente.
