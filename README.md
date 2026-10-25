# Óxido

Óxido es un proyecto de lenguaje con sintaxis bilingüe que busca generar Rust estándar y delegar la semántica y compilación a Rust/Cargo.

## Estado

El proyecto está en fase inicial. Hoy existe una base Cargo, un lexer de `.ox` como biblioteca y un comando experimental para analizar Rust como comprobación del pipeline. El parser y compilador `.ox` aún no están implementados. La configuración `oxido.toml` se acepta como TOML de datos con campos cerrados; todavía no se descubre ni carga automáticamente desde el workspace. Consulta el [portal documental](docs/index.md), el [roadmap](docs/roadmap.md) y la [especificación](docs/language-spec.md). Para trabajar en el sitio Markdown, sigue [website/README.md](website/README.md).

## Requisitos

- Rust estable; la edición objetivo del workspace es Rust 2024.
- Cargo.

## Comprobación inicial

```sh
cargo run --locked -p oxido-cli -- analizar-rust ruta/al/archivo.rs
```

El comando valida que el archivo sea sintácticamente Rust válido. No compila ni ejecuta el programa.

## Desarrollo

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

El portal se construye desde `website/` con `npm ci && npm run build` (Node 22.23.4).

Lee [AGENTS.md](AGENTS.md), [reglas de arquitectura](ARCHITECTURE_RULES.md) y [plan](plan) antes de modificar el lenguaje o su arquitectura.
