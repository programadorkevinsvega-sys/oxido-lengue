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

| Proyecto | Fuente raíz / modo | Configuración y dependencias | Ejecución y estado |
|---|---|---|---|
| Ejemplos básicos | `examples/hola_mundo.ox`, indentación | Sin dependencias del programa ni manifiesto/lockfile Cargo propio; el CLI se ejecuta desde el workspace. | `cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox`; CI: prueba `exports_compiles_and_runs_the_hello_world_fixture`, salida exacta `Hola, mundo.`. `fase_7_estructuras`, `fase_7_interoperabilidad`, `fase_7_prelude_espanol` y `fase_8_estructura_indentada` tienen pruebas de pipeline; alcance en [`examples/README.md`](../examples/README.md). |
| Interoperabilidad con `std` | `prueba/interoperabilidad_std/main.ox`, indentación | `prueba/interoperabilidad_std/oxido.toml`; sin crates externos ni Cargo.lock propio. | El comando `cargo run --locked -p oxido-cli -- ejecutar prueba/interoperabilidad_std/main.ox` ejecuta las aserciones. Procedimiento documentado en [`prueba/README.md`](../prueba/README.md); proyecto manual, no test automático de CI. |
| Módulos jerárquicos | `prueba/modulos_jerarquicos/fuente/main.ox`, **llaves** | `prueba/modulos_jerarquicos/oxido.toml`; sin dependencias externas. `rust/Cargo.lock` y `rust-recursivo/Cargo.lock` fijan las exportaciones Rust. | La prueba CLI `compiles_runs_and_recursively_exports_a_hierarchical_ox_project` es automática; salida esperada `Ana aprobó: true`. Instrucciones y límite de importación en [`prueba/modulos_jerarquicos/README.md`](../prueba/modulos_jerarquicos/README.md). |
| CLI interactiva | `prueba/terminal-cli-oxido-py/src/main.ox`, indentación | `prueba/terminal-cli-oxido-py/oxido.toml`; sin dependencias externas ni Cargo.lock propio. | Exportación y compilación con `rustc` y tests manuales descritos en [`prueba/terminal-cli-oxido-py/README.md`](../prueba/terminal-cli-oxido-py/README.md); no está automatizado en CI. |
| CRUD concurrente | `prueba/crud_api_concurrencia.ox` y `prueba/verificar_api_concurrencia.ox`, indentación | `prueba/oxido.toml` y `prueba/Cargo.lock`; Axum, Serde y Tokio. | Servidor y cliente corren en procesos separados; el cliente genera solicitudes simultáneas. La guía registra los casos CRUD/concurrencia en [`prueba/README.md`](../prueba/README.md). Ejecución manual; no es un benchmark de rendimiento. |
| CRUD persistente | `prueba/crud_sqlite/crud_api_persistencia.ox` y `verificar_persistencia.ox`, indentación | `prueba/crud_sqlite/oxido.toml` y `Cargo.lock`; Axum, Serde, Tokio y SQLx 0.8.6 con SQLite. | Requiere iniciar/detener el servidor entre fases y conservar la misma ruta de BD. Pasos, resultados observables y medición exploratoria en [`prueba/crud_sqlite/README.md`](../prueba/crud_sqlite/README.md). Ejecución manual. |
| Línea base y exportación Rust | `prueba/crud_sqlite_rust/` y `prueba/crud_sqlite_export_rust/`, Rust nativo y exportado | Cada proyecto tiene `Cargo.toml` y `Cargo.lock`; Axum, Serde, Tokio y SQLx 0.8.6 con features alineadas. | El supervisor corre ambos servidores secuencialmente y usa el mismo cliente. Procedimientos en sus README; benchmarks manuales, no se ejecutan en CI ni prueban rendimiento de producción. |

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
Los informes locales que deban conservarse como evidencia histórica se revisan y archivan en `docs/reports/`; el reporte de la campaña extendida de Fase 10 es [`2026-10-26-10000-por-target.md`](../docs/reports/fase-10/2026-10-26-10000-por-target.md).

El fuzzing usa `cargo-fuzz` 0.13.2 y Rust nightly 2026-10-16, fijados en el job `fuzz-smoke` de CI:

```sh
cargo +nightly-2026-10-16 fuzz run lexer -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run parser -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run config -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run backend -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run lsp -- -runs=1000 -max_len=8192
```

Para ejecutar campañas más largas en los cinco targets y registrarlas en el informe local (10.000 iteraciones por target):

```sh
OXIDO_VALIDATION_FUZZ_RUNS=10000 cargo run --manifest-path tools/validation-report/Cargo.toml --locked
```

El override admite de 1.000 a 10.000.000 iteraciones; si no se define, conserva el valor CI de 1.000. El comando realiza también el resto de verificaciones del recolector. Las campañas no tienen objetivo de cobertura implícito; informa el número configurado, el commit y el entorno.

Para investigar un fallo, conserva el artefacto, minimízalo con `cargo fuzz tmin <target> <artefacto>` y añade el caso mínimo al corpus versionado y a una prueba de regresión determinista apropiada. No reportes como fuzzing prolongado un smoke de 1.000 ejecuciones. Los informes de ejecución deben distinguir aprobadas, fallidas y no evaluadas; cualquier cifra de rendimiento exige entorno, carga, repeticiones y método medido.

El inventario del estado inicial y los pendientes de Fase 10 está en [`docs/fase-10-laboratorio-validacion.md`](../docs/fase-10-laboratorio-validacion.md).
