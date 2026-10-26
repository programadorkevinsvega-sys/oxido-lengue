# Laboratorio de validación

La mayoría de las pruebas vive junto al crate que valida:

- `crates/oxido-language/tests/`: lexer, parser, spans, Unicode, modos, límites y diagnósticos.
- `crates/oxido-compiler/tests/`: configuración, generación, integración con rustc/Cargo, modos, aliases y roundtrip.
- `crates/oxido-cli/tests/`: flujos de comandos reales, proyectos de referencia, importación/exportación y formato.
- `crates/oxido-rust-backend/`: pruebas unitarias del emisor, normalización y mapas.
- `tests/fixtures/cargo-registry/`: crates fijadas y vendor local para integración Cargo repetible sin depender de la caché personal.
- `tests/fixtures/cargo-execution/`: fixtures controlados con `build.rs` y macro procedural para probar la política de confianza.
- `fuzz/`: harnesses `cargo-fuzz` y corpus semilla versionado para lexer, parser, backend, dispatcher LSP y `oxido.toml`; los inputs que encuentre el fuzzing se ignoran localmente hasta que se revisen y se añadan como regresiones.
- `examples/` y `prueba/`: ejemplos ejecutables y proyectos de referencia funcionales; no sustituyen las pruebas del compilador.

### Contrato para añadir una referencia

Cada proyecto de `examples/` o fixture de integración debe documentar: archivo raíz, modo de sintaxis, comando exacto, dependencias y lockfile, resultado observable esperado, estado (aprobado/fallido/no evaluado) y límites que no cubre. Si requiere servicios o estado persistente, el harness debe iniciar y detener los procesos de forma secuencial, aislar los datos por ejecución y verificar limpieza sin descartar errores. Las pruebas de referencia no deben presentarse como mediciones de rendimiento salvo que registren entorno, calentamiento, concurrencia, rondas y latencias/throughput medidos.

## Comandos reproducibles

Desde la raíz del repositorio:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
```

El fuzzing usa `cargo-fuzz` 0.13.2 y Rust nightly 2026-10-16, fijados en el job `fuzz-smoke` de CI:

```sh
cargo +nightly-2026-10-16 fuzz run lexer -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run parser -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run config -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run backend -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run lsp -- -runs=1000 -max_len=8192
```

Para investigar un fallo, conserva el artefacto, minimízalo con `cargo fuzz tmin <target> <artefacto>` y añade el caso mínimo al corpus versionado y a una prueba de regresión determinista apropiada. No reportes como fuzzing prolongado un smoke de 1.000 ejecuciones. Los informes de ejecución deben distinguir aprobadas, fallidas y no evaluadas; cualquier cifra de rendimiento exige entorno, carga, repeticiones y método medido.

El inventario del estado inicial y los pendientes de Fase 10 está en [`docs/fase-10-laboratorio-validacion.md`](../docs/fase-10-laboratorio-validacion.md).
