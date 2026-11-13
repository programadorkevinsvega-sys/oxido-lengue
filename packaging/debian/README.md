# Paquete Debian de Óxido

## Preflight de la toolchain del usuario

`check-rust-toolchain.sh` solo informa: requiere `rustc` y `cargo` estables 1.99.1 o posteriores y, si `rustfmt` está instalado, verifica que su commit corresponda al de `rustc`. No instala, actualiza ni desinstala Rust. Una versión inferior falla e indica la versión detectada y la mínima requerida.

## Instalación de toolchain aislada

`install-isolated-toolchain.sh` instala Rust 1.99.1 debajo de `/opt/oxido` por defecto. Acepta `--prefix DIR` para pruebas o una ubicación administrada por el sistema. Guarda rustup en `DIR/rustup` y Cargo en `DIR/cargo`, y fija `RUSTUP_HOME`/`CARGO_HOME` solo para el proceso de instalación. Usa `--no-modify-path`; no toca `~/.rustup`, `~/.cargo`, archivos de inicio de shell ni el PATH global.

El bootstrap está fijado a rustup-init 1.29.1. El script acepta solo Debian amd64/arm64 (targets `x86_64-unknown-linux-gnu` y `aarch64-unknown-linux-gnu`) y valida el SHA-256 del binario antes de ejecutarlo:

| Arquitectura | SHA-256 de `rustup-init` |
| --- | --- |
| amd64 | `dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71` |
| arm64 | `15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433` |

Los hashes se contrastaron descargando los binarios del archivo oficial de Rustup. La descarga de Rust queda sujeta al manifiesto y checksums que valida Rustup. El proceso tiene un límite de 30 minutos.

El preflight informa si la toolchain del usuario resulta compatible, pero el bootstrap instala en paralelo su propia toolchain y no bloquea ni sustituye la del sistema. Una instalación previa en el prefijo de Óxido se valida y se reutiliza si cumple el mínimo; una instalación antigua, incoherente o parcial se rechaza sin reemplazarla. Los errores de descarga, checksum o instalación eliminan solo los directorios parciales creados por esa ejecución. Los requisitos del host son Bash, `curl`, `coreutils` y `awk`.

Ejemplo de instalación del sistema:

```sh
sudo packaging/debian/install-isolated-toolchain.sh
```

La herramienta informa las variables necesarias para usarla en una sesión. La integración automática de esas variables con el paquete `.deb` pertenece a otra subfase.

## Pruebas

Las regresiones simuladas no descargan ni ejecutan binarios de terceros:

```sh
bash packaging/debian/tests/check-rust-toolchain.sh
bash packaging/debian/tests/install-isolated-toolchain.sh
```

El script de verificación se puede invocar directamente:

```sh
bash packaging/debian/check-rust-toolchain.sh
```

El instalador de toolchain es un comando explícito, separado de los scripts mantenedores de Debian; `preinst`/`postinst` no descargan ni ejecutan Rustup. Todavía no se genera un `.deb`.

Referencias: [instalación manual y checksums de rustup-init](https://rust-lang.github.io/rustup/installation/other) y [opciones de rustup-init](https://github.com/rust-lang/rustup/blob/master/doc/user-guide/src/installation/other.md).
