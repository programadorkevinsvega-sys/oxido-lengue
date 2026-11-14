# Clonar, compilar y contribuir

Esta guía es para quienes quieren trabajar en el código fuente de Óxido. **No hace falta seguirla para usar el lenguaje.** Para instalar la CLI publicada y crear proyectos, sigue [Instalación y primeros pasos](instalacion.md).

## Requisitos

- Git.
- Rust 1.99.1 o posterior, Cargo y `rustfmt` del mismo toolchain.
- Node.js y npm para compilar la documentación o trabajar en la extensión de VS Code.

## Clonar y compilar la CLI

```sh
git clone https://github.com/programadorkevinsvega-sys/oxido-lengue.git
cd oxido-lengue
cargo build --release --locked -p oxido-cli
./target/release/oxido --version
```

Para usar la CLI desde el repositorio sin instalarla en el sistema:

```sh
cargo run --locked -p oxido-cli -- --help
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```

También se puede instalar el binario compilado desde el checkout actual:

```sh
cargo install --path crates/oxido-cli --locked
oxido --version
```

`cargo install` instala la CLI en el `PATH` de Cargo del usuario. No instala ni actualiza la Release Debian.

## Verificar cambios

Antes de enviar una contribución, ejecuta las comprobaciones pertinentes. La suite del workspace se verifica con:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
```

Lee [`CONTRIBUTING.md`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/CONTRIBUTING.md), [`AGENTS.md`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/AGENTS.md), [`ARCHITECTURE_RULES.md`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/ARCHITECTURE_RULES.md) y la documentación técnica en `docs/` antes de cambiar el compilador. Cada cambio de comportamiento debe incluir regresiones y evidencia reproducible. Los paquetes Debian y la extensión VS Code se publican por separado; compilar desde Git no modifica ninguna Release.

## Desinstalar una instalación local

Si instalaste la CLI con `cargo install`, puedes quitarla con:

```sh
cargo uninstall oxido-cli
```

Esto no afecta instalaciones realizadas con el paquete Debian. Para esa ruta, consulta [desinstalar Óxido](instalacion.md#desinstalar).
