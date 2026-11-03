# Óxido

Óxido es un proyecto de lenguaje con sintaxis bilingüe que busca generar Rust estándar y delegar la semántica y compilación a Rust/Cargo.

## Estado

El proyecto ya tiene lexer, parser y generación Rust para el subconjunto documentado. La CLI exporta, compila y ejecuta ejemplos; el editor con LSP ofrece diagnósticos y, en un workspace confiable, navegación con `rust-analyzer`. Hay pruebas diferenciales, fuzzing, benchmarks y builds reproducibles, pero no son una auditoría completa. La compatibilidad sigue limitada a la [matriz publicada](docs/compatibility.md) y no es una herramienta lista para producción: la ejecución de código de dependencias requiere confianza externa y no está sandboxeada. La CLI lee `oxido.toml` como datos declarativos con campos cerrados. El sandbox del playground solo se verificó en Linux y no está desplegado. Consulta el [portal documental](docs/index.md), el [tutorial](docs/tutorial.md) y la [especificación](docs/language-spec.md). Para trabajar en el sitio Markdown, sigue [website/README.md](website/README.md).

## Requisitos

- Rust 1.99.1 o posterior; la edición objetivo del workspace es Rust 2024. CI usa Rust 1.99.1 para formato, lint, pruebas y auditoría de dependencias.
- Cargo.

## Primeros pasos

```sh
cargo install --path crates/oxido-cli --locked
oxido nuevo demo
oxido ejecutar demo/src/main.ox
```

La guía completa —requisitos, comandos, dependencias y solución de problemas— está en [docs/instalacion.md](docs/instalacion.md). El [tutorial progresivo](docs/tutorial.md) ofrece tres lecciones ejecutables y el [libro de ejercicios](docs/ejercicios.md) incluye soluciones verificadas y tareas propuestas. Para el editor, consulta [docs/vscode.md](docs/vscode.md). El material educativo está en [docs/education.md](docs/education.md), el protocolo de pilotos en [docs/pilotos-educativos.md](docs/pilotos-educativos.md), la guía de inicio en inglés en [docs/en/quickstart.md](docs/en/quickstart.md) y los criterios de evaluación empresarial en [docs/enterprise-readiness.md](docs/enterprise-readiness.md).

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

## Licencia

El código se distribuye con doble licencia [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), a elección de quien lo use. Las contribuciones se aceptan bajo los mismos términos. Los ejemplos de `examples/` y la documentación de `docs/` se incluyen bajo la misma licencia salvo que se indique lo contrario.
