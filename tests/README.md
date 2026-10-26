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
| Interoperabilidad con `std` | `prueba/interoperabilidad_std/main.ox`, indentación | `prueba/interoperabilidad_std/oxido.toml`; sin crates externos ni Cargo.lock propio. | `reference-runner` ejecuta el proyecto en CI y el comando `cargo run --locked -p oxido-cli -- ejecutar prueba/interoperabilidad_std/main.ox` lo ejecuta localmente. |
| Módulos jerárquicos | `prueba/modulos_jerarquicos/fuente/main.ox`, **llaves** | `prueba/modulos_jerarquicos/oxido.toml`; sin dependencias externas. `rust/Cargo.lock` y `rust-recursivo/Cargo.lock` fijan las exportaciones Rust. | La prueba CLI `compiles_runs_and_recursively_exports_a_hierarchical_ox_project` exporta/compila/ejecuta ambos árboles en CI; el job de referencias también ejecuta los dos manifiestos versionados con salida `Ana aprobó: true`. Límite de importación en [`prueba/modulos_jerarquicos/README.md`](../prueba/modulos_jerarquicos/README.md). |
| CLI de terminal | `prueba/terminal-cli-oxido-py/src/main.ox`, indentación; línea base `prueba/terminal-cli-rust/` | `oxido.toml` y Cargo.lock sin dependencias externas. | `reference-regressions` exporta el árbol `.ox`, ejecuta los tres tests del parser y corre tests/binario de Rust nativo. Un guion compartido ejercita `help`, `echo`, errores, `pwd`, `ls`, `cat`, `cd` y `exit` contra ambos binarios en un directorio temporal; compara stdout/stderr exactos. Instrucciones en [`prueba/terminal-cli-oxido-py/README.md`](../prueba/terminal-cli-oxido-py/README.md). |
| CRUD concurrente | `prueba/crud_api_concurrencia.ox` y `prueba/verificar_api_concurrencia.ox`, indentación | `prueba/oxido.toml` y `prueba/Cargo.lock`; Axum, Serde y Tokio. | `reference-runner` inicia el servidor en grupo de procesos aislado y ejecuta en CI las validaciones CRUD, lotes totales de 100/500/1.000 altas con un máximo de 100 solicitudes simultáneas por ola, y el caso separado de desbordamiento. No calcula métricas de rendimiento. |
| CRUD persistente exportado | `prueba/crud_sqlite_export_rust/`, generado desde los `.ox` de `prueba/crud_sqlite/` | `Cargo.toml`/`Cargo.lock`; Axum, Serde, Tokio y SQLx 0.8.6 con SQLite. | El job `reference-regressions` exporta, compila con Cargo y corre el verificador contra reinicios reales, altas concurrentes, conflicto UNIQUE, actualización y eliminación. La prueba escribe en SQLite temporal del runner. |
| API SQLite directa y línea base | `prueba/crud_sqlite/` (`.ox`) y `prueba/crud_sqlite_rust/` (Rust nativo) | `oxido.toml`/`Cargo.lock` y `Cargo.toml`/`Cargo.lock`; Axum, Serde, Tokio y SQLx 0.8.6 con features alineadas. | `reference-regressions` inicia los dos servidores secuencialmente y valida HTTP sin enviar la carga del benchmark. El benchmark automatizado semanal y el `workflow_dispatch` ejecutan la comparación medida; ver README de la línea base. |

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
cargo run --manifest-path tools/validation-report/Cargo.toml --bin oxido-validation-report --locked
```

El reporte queda en `target/validation-report.md`. Las herramientas opcionales no instaladas quedan como **No evaluada**; cualquier verificación que sí se ejecuta y falla aparece como **Fallida** y hace fallar el comando.
En GitHub Actions, el job `validation-report` ejecuta el mismo recolector con sus herramientas instaladas y publica `validation-report-<commit>` como artefacto por 30 días, incluso cuando alguna suite falla y alcanza a escribir el reporte.
Los informes locales que deban conservarse como evidencia histórica se revisan y archivan en `docs/reports/`; el reporte de la campaña extendida de Fase 10 es [`2026-10-26-10000-por-target.md`](../docs/reports/fase-10/2026-10-26-10000-por-target.md).

El job `reference-regressions` ejecuta las referencias con servicios en procesos secuenciales. El benchmark comparativo release se ejecuta cada lunes en GitHub Actions y se puede iniciar también desde GitHub Actions → CI → Run workflow → `ejecutar_benchmark`. El log completo se publica como `oxido-rust-benchmark-<commit>`. Esa ejecución mide únicamente la carga definida por el harness y no prueba rendimiento de producción; este cambio no ejecutó ni midió el benchmark.

El fuzzing usa `cargo-fuzz` 0.13.2 y Rust nightly 2026-10-16, fijados en el job `fuzz-smoke` de CI:

```sh
cargo +nightly-2026-10-16 fuzz run lexer -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run parser -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run config -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run backend -- -runs=1000 -max_len=8192
cargo +nightly-2026-10-16 fuzz run lsp -- -runs=1000 -max_len=8192
```

Para ejecutar una campaña extendida en los cinco targets y registrarla en el informe local (10.000 iteraciones por target):

```sh
OXIDO_VALIDATION_FUZZ_RUNS=10000 cargo run --manifest-path tools/validation-report/Cargo.toml --bin oxido-validation-report --locked
```

El override admite de 1.000 a 10.000.000 iteraciones; si no se define, conserva el smoke CI de 1.000. En el workflow semanal, el recolector fija 10.000 por target; en push/PR mantiene 1.000. El comando realiza también el resto de verificaciones del recolector. Las campañas no tienen objetivo de cobertura implícito; informa el número configurado, el commit y el entorno.

Para investigar un fallo, conserva el artefacto, minimízalo con `cargo fuzz tmin <target> <artefacto>` y añade el caso mínimo al corpus versionado y a una prueba de regresión determinista apropiada. No reportes como fuzzing prolongado un smoke de 1.000 ejecuciones. Los informes de ejecución deben distinguir aprobadas, fallidas y no evaluadas; cualquier cifra de rendimiento exige entorno, carga, repeticiones y método medido.

El inventario del estado inicial y los pendientes de Fase 10 está en [`docs/fase-10-laboratorio-validacion.md`](../docs/fase-10-laboratorio-validacion.md).
