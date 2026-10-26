# Laboratorio de validación

La mayoría de las pruebas vive junto al crate que valida:

- `crates/oxido-language/tests/`: lexer, parser, spans, Unicode, modos, límites y diagnósticos.
- `crates/oxido-compiler/tests/`: configuración, generación, integración con rustc/Cargo, modos, aliases y roundtrip.
- `crates/oxido-cli/tests/`: flujos de comandos reales, proyectos de referencia, importación/exportación y formato.
- `crates/oxido-rust-backend/`: pruebas unitarias del emisor, normalización y mapas.
- `tools/validation-report/`: ejecutor Rust local que consolida suites, herramientas/versions y estados en un reporte Markdown.
- `tests/fixtures/cargo-registry/`: crates fijadas y vendor local para integración Cargo repetible sin depender de la caché personal.
- `tests/fixtures/cargo-execution/`: fixtures controlados con `build.rs` y macro procedural para probar la política de confianza.
- `fuzz/`: harnesses `cargo-fuzz` y corpus semilla versionado para lexer, parser, backend, framing/dispatcher LSP y `oxido.toml`; los inputs que encuentre el fuzzing se ignoran localmente hasta que se revisen y se añadan como regresiones.
- `examples/` y `prueba/`: ejemplos ejecutables y proyectos de referencia funcionales; no sustituyen las pruebas del compilador.

### Contrato para añadir una referencia

Cada proyecto de `examples/` o fixture de integración debe documentar: archivo raíz, modo de sintaxis, comando exacto, dependencias y lockfile, resultado observable esperado, estado (aprobado/fallido/no evaluado) y límites que no cubre. Si requiere servicios o estado persistente, el harness debe iniciar y detener los procesos de forma secuencial, aislar los datos por ejecución y verificar limpieza sin descartar errores. Las pruebas de referencia no deben presentarse como mediciones de rendimiento salvo que registren entorno, calentamiento, concurrencia, rondas y latencias/throughput medidos.

### Índice de proyectos de referencia

| Proyecto | Fuente raíz / modo | Evidencia y estado |
|---|---|---|
| Ejemplos básicos | `examples/hola_mundo.ox`, indentación | La CI compila/ejecuta la prueba CLI `exports_compiles_and_runs_the_hello_world_fixture`; otros ejemplos tienen instrucciones en [`examples/README.md`](../examples/README.md) y sus límites se describen allí. |
| Interoperabilidad con `std` | `prueba/interoperabilidad_std/main.ox`, indentación | Proyecto autónomo con aserciones ejecutables; pasos y alcance en [`prueba/README.md`](../prueba/README.md). |
| Módulos jerárquicos | `prueba/modulos_jerarquicos/fuente/main.ox`, indentación | La regresión CLI compara compilación/exportación y ejecuciones. El README registra que la importación de declaraciones y módulos no está soportada. |
| CLI interactiva | `prueba/terminal-cli-oxido-py/src/main.ox`, indentación | Guía de exportación, compilación y tests manuales en [`prueba/terminal-cli-oxido-py/README.md`](../prueba/terminal-cli-oxido-py/README.md); no se presenta como suite automática del workspace. |
| CRUD concurrente | `prueba/crud_api_concurrencia.ox` y `prueba/verificar_api_concurrencia.ox`, indentación | Requiere iniciar servidor y cliente en procesos separados; procedimiento y límites en [`prueba/README.md`](../prueba/README.md). No es un benchmark de rendimiento. |
| CRUD persistente | `prueba/crud_sqlite/crud_api_persistencia.ox` y `verificar_persistencia.ox`, indentación | Requiere SQLite vía SQLx, lockfile y reinicios secuenciales; pasos, estados observables y medición exploratoria en [`prueba/crud_sqlite/README.md`](../prueba/crud_sqlite/README.md). |
| Línea base y exportación Rust | `prueba/crud_sqlite_rust/` y `prueba/crud_sqlite_export_rust/`, Rust generado / Rust nativo | Supervisor de benchmark secuencial y regresión exportada documentados en sus respectivos README. Son ejecuciones manuales costosas; sus mediciones no forman parte de cada test del workspace. |

Las pruebas automatizadas exactas por capacidad están en la [matriz de trazabilidad](../docs/testing.md#matriz-de-trazabilidad-de-capacidades-anunciadas). Los procedimientos manuales no se cuentan como aprobados por CI.

## Comandos reproducibles

Desde la raíz del repositorio:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
```

Para ejecutar el conjunto registrado y generar un Markdown con commit, entorno y resultados por suite:

```sh
cargo run --manifest-path tools/validation-report/Cargo.toml --locked
```

El reporte queda en `target/validation-report.md`. Las herramientas opcionales no instaladas quedan como **No evaluada**; cualquier verificación que sí se ejecuta y falla aparece como **Fallida** y hace fallar el comando.

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
