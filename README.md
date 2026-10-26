# Óxido

Óxido es un proyecto de lenguaje con sintaxis bilingüe que busca generar Rust estándar y delegar la semántica y compilación a Rust/Cargo.

## Estado

El proyecto ya tiene lexer, parser y generación Rust. Las Fases 5–9 están completadas en sus alcances verificados; la Fase 10 está en curso e incorpora diferenciales iniciales y fuzzing acotado de lexer, parser, backend, configuración y dispatcher LSP. La Fase 8 incluye estructura indentada e interoperabilidad con dependencias Cargo locales y de crates.io, con lockfile explícito y compilación offline; la Fase 9 incluye CLI, cobertura estructural de formato y Rust exportado alineado carácter por carácter con `rustfmt` 1.99.1 en los casos documentados. La compatibilidad sigue limitada a lo documentado y no es una herramienta lista para producción; la ejecución de código de dependencias aún requiere confianza externa y no está sandboxeada. La CLI lee `oxido.toml` como datos declarativos con campos cerrados. Consulta el [portal documental](docs/index.md), el [roadmap](docs/roadmap.md) y la [especificación](docs/language-spec.md). Para trabajar en el sitio Markdown, sigue [website/README.md](website/README.md).

## Requisitos

- Rust 1.99.1 o posterior; la edición objetivo del workspace es Rust 2024. CI usa Rust 1.99.1 para formato, lint, pruebas y auditoría de dependencias.
- Cargo.

## Ejemplo ejecutable inicial

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```

`exportar` crea también `target/hola.oxido-map.json`. `compilar` usa Cargo para comprobar el Rust generado; `ejecutar` es la única orden que corre el programa `.ox`. La importación reversible se limita a funciones libres admitidas:

```sh
cargo run --locked -p oxido-cli -- importar target/hola.rs --salida target/hola-vuelta.ox
```

El comando `analizar-rust` sigue disponible para validar sintaxis Rust directamente.

## Desarrollo

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

El portal se construye desde `website/` con `npm ci && npm run build` (Node 22.23.4).

Lee [AGENTS.md](AGENTS.md), [reglas de arquitectura](ARCHITECTURE_RULES.md) y [plan](plan) antes de modificar el lenguaje o su arquitectura.
