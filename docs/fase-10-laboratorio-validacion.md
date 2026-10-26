# Fase 10 — Laboratorio de validación

**Estado:** en curso. Esta fase consolida evidencia reproducible; no amplía por sí sola la compatibilidad del lenguaje.

## Inventario inicial

| Subfase | Estado inicial verificado | Evidencia disponible / trabajo pendiente |
|---|---|---|
| 1. Matriz por componente | Parcial | Lexer y parser en `oxido-language`; configuración y pipeline en `oxido-compiler`; formato/macros/CLI en `oxido-cli`; la matriz explícita de características y tests sigue creciendo. |
| 2. Regresiones | Implementada en el corpus existente | Los defectos corregidos tienen pruebas en el crate afectado; fuzz crashes futuros deben añadirse como seed y regresión determinista. |
| 3. Diferencial contra Rust | Parcial, con primera regresión | `differential_indented_ox_and_equivalent_rust_have_the_same_observable_result` compila y ejecuta un programa indentado `.ox` y un equivalente Rust con `rustc`, y compara stdout. Hace falta ampliar expresiones/errores y mantener toolchains alineados. |
| 4. Ida y vuelta | Implementada para subconjuntos probados | CLI verifica `.ox → .rs → .ox → .rs` y roundtrip de árboles mixtos; no equivale a roundtrip universal. |
| 5. Fuzzing | Implementación inicial | Targets de lexer, parser, configuración TOML, backend/mapas y framing/dispatcher JSON-RPC LSP; corpus semilla y smoke CI. Las campañas siguen acotadas. |
| 6. Límites y recuperación | Parcial | Hay pruebas de tamaños máximos, anidamiento, Unicode, spans, recuperación de parser y límites LSP de cabecera 8 KiB/frame 4 MiB; faltan recuperación sistemática, cancelación LSP y reportes de límites medidos. |
| 7. Proyectos reutilizables | Parcial | `tests/README.md` define un contrato mínimo para nuevas referencias; los proyectos previos aún no están todos normalizados a ese contrato. |
| 8. Informes reproducibles | Pendiente | Este informe registra herramienta/commit/entorno y estados; no hay aún recolector de informes por ejecución ni benchmark reproducible del compilador. No se inventan métricas de rendimiento. |
| 9. CI | En progreso | CI valida fmt, Clippy, workspace, cargo-deny y smoke de fuzzing; los jobs remotos pasaron para el commit registrado abajo. Falta mantener esta evidencia en siguientes iteraciones. |

## Harnesses de fuzzing

Los targets y corpus están en `fuzz/`; instrucciones locales y política para promover entradas halladas están en [`tests/README.md`](../tests/README.md). El harness aplica propiedades de seguridad a spans además de la propiedad de no panicar. Ignora bytes que no sean UTF-8 válido porque las fuentes `.ox` se representan como `str`; sí incluye Unicode válido y controles bidireccionales como entrada.

Comandos de smoke ejecutados en el entorno local:

```sh
cargo +nightly fuzz build
cargo +nightly fuzz run lexer -- -runs=1000 -max_len=8192
cargo +nightly fuzz run parser -- -runs=1000 -max_len=8192
cargo +nightly fuzz run config -- -runs=1000 -max_len=8192
cargo +nightly fuzz run backend -- -runs=1000 -max_len=8192
cargo +nightly fuzz run lsp -- -runs=1000 -max_len=8192
```

Resultado inicial: los tres targets terminaron 1.000 iteraciones sin crash ni violación de span. Esto demuestra que los harnesses corren y cubren una muestra corta; no demuestra que el parser esté libre de defectos ni constituye una campaña larga. Los inputs mutacionales generados por libFuzzer se ignoran localmente. Un artefacto sospechoso se minimiza, revisa y convierte en seed versionado y regresión determinista antes de aceptarlo.

## Registro inicial reproducible

| Dato | Valor |
|---|---|
| Fecha | 2026-10-26 |
| Commit base antes de incorporar esta implementación | `af85cc5` |
| Entorno | Linux x86_64 local |
| Toolchain estable del proyecto | Rust 1.99.1 |
| Toolchain del fuzzing | rustc 1.101.0-nightly (`5d8fec066`, 2026-10-16) |
| `cargo-fuzz` | 0.13.2 |
| Smoke por target | 1.000 ejecuciones, máximo 8 KiB por entrada |
| Resultado | Lexer, parser y config: aprobados localmente antes de añadir backend; backend: pendiente de smoke local; CI remota del job: pendiente |
| Rendimiento del compilador | No medido en este registro |

Este registro describe la primera ejecución de la Fase 10 antes del commit de estos cambios. La prueba diferencial añadida después cubre un único programa y no debe extrapolarse a equivalencia universal. Para cada campaña posterior deben anotarse el commit exacto, host/toolchain, configuración, comando, número/duración de ejecuciones, aprobadas/fallidas/no evaluadas y artefactos/regresiones promovidos. No extrapolar QPS de las pruebas de API al rendimiento del compilador o del lenguaje.

## Registro de esta iteración

| Dato | Valor |
|---|---|
| Commit implementado y probado | `1911824a9fc8e948d0212957689f079bb2068dee` |
| Toolchain workspace | Rust/Cargo 1.99.1; Linux x86_64 |
| Diferencial | `differential_indented_ox_and_equivalent_rust_have_the_same_observable_result`: aprobado; un programa compilado/ejecutado en ambas rutas, stdout idéntico |
| Workspace | `cargo fmt --all -- --check`: aprobado; `cargo clippy --workspace --all-targets --locked -- -D warnings`: aprobado; `cargo test --workspace --locked`: 130 pruebas aprobadas, 0 fallidas |
| Fuzz | Lexer, parser, config y backend: 1.000 iteraciones cada uno, máximo 8 KiB; aprobados localmente sin crash ni spans inválidos |
| Compilación fuzz | `cargo +nightly check --manifest-path fuzz/Cargo.toml --bins --locked` y `cargo +nightly fuzz build backend`: aprobados |
| Dependencias | `cargo deny check`: exit 0; mostró diagnósticos no fatales `unresolved-workspace-dependency` y `wildcard` para dependencias path/workspace locales |
| CI remota | Aprobada en push y pull request: `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages |
| Rendimiento | No medido |

Comandos reproducibles del workspace y fuzzing están en [`tests/README.md`](../tests/README.md). La CI corresponde al commit indicado y sus mismos contenidos de implementación. Este documento registra la evidencia disponible, pero no cierra la fase: siguen pendientes diferenciales más amplios, decoder/cancelación LSP y la revisión final de cobertura por característica.

## Iteración diferencial posterior

| Dato | Valor |
|---|---|
| Base | Merge de la primera entrega de Fase 10: `9920b15dc33fe7067acf356bdeac1142df54f306` |
| Diferenciales | Dos programas `.ox` indentados y equivalentes en Rust nativo; ambos compilan y ejecutan, stdout coincide |
| Construcciones comparadas | Iteradores/colección/condicional y `struct`/`impl`/método/`match` |
| Verificación local | Antes de extraer el dispatcher LSP: `cargo fmt --all -- --check`, Clippy y `cargo test --workspace --locked`: 131 pruebas aprobadas, 0 fallidas. La extensión LSP añade 4 pruebas unitarias; suite completa pendiente de repetir antes del commit |
| CI remota de esta iteración | Pendiente al registrar; actualizar con el commit y checks cuando termine |
| Límites | Solo dos fixtures; no cubre equivalencia semántica general ni diagnostica pares de errores equivalentes |

## Iteración de LSP y corpus JSON-RPC

| Dato | Valor |
|---|---|
| Commit base | `f04f34fca39f5e6b19ac620534d3d672bf23e17e` |
| Cambios | Dispatcher LSP aislado en biblioteca; decoder stdio con cabeceras de hasta 8 KiB y `Content-Length` hasta 4 MiB; rechaza longitud duplicada y cubre faltante/truncamiento; quinto target `cargo-fuzz` para el stream y dispatcher |
| Verificación local | fmt, Clippy y `cargo test --workspace --locked`: 138 pruebas aprobadas, 0 fallidas; `cargo +nightly check --manifest-path fuzz/Cargo.toml --bins --locked`: aprobado |
| Smoke LSP | `cargo +nightly fuzz run lsp -- -runs=1000 -max_len=8192`: 1.000 iteraciones, sin crash |
| Commit implementado y probado | `46fa7d73bf4901b1b00ea4a20f57fe9327eedf5c` |
| CI remota | Aprobada para el commit registrado: `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages, tanto en push como en pull request |
| Límites | No hay cancelación LSP, campaña prolongada ni pruebas de recuperación incremental del documento |

## Iteración de framing LSP

| Dato | Valor |
|---|---|
| Base | `46fa7d73bf4901b1b00ea4a20f57fe9327eedf5c` |
| Decoder | El fuzzer consume bytes del framing `Content-Length` y despacha payloads válidos; máximo 8 KiB por línea de cabecera y 4 MiB por frame; longitudes duplicadas se rechazan |
| Regresiones de transporte | Frame válido, EOF, Content-Length ausente/duplicado, cabecera demasiado larga, frame sobredimensionado y cuerpo truncado |
| Verificación local | `cargo fmt --all -- --check`, Clippy y `cargo test --workspace --locked`: 138 pruebas aprobadas, 0 fallidas; fuzz manifiesto chequeado con `--locked` |
| Smoke LSP | `cargo +nightly fuzz run lsp -- -runs=1000 -max_len=8192`: 1.000 ejecuciones, sin crash |
| CI remota | Pendiente para el commit de esta iteración |
| Límites | La cancelación LSP y la recuperación incremental siguen sin implementar/probar |

## Criterio de cierre

La fase seguirá abierta hasta que las características publicadas en la matriz tengan tests identificables, las limitaciones y campañas no ejecutadas figuren como pendientes, los diferenciales/roundtrips relevantes estén automatizados y el job de CI de fuzzing pase. No se exige demostrar exhaustivamente toda la gramática de Rust ni trasladar a este laboratorio la aceptación funcional de crates, macros, runtimes o `unsafe`; esa decisión corresponde a la Fase 8 según ADR-0007.
