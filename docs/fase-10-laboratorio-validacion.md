# Fase 10 — Laboratorio de validación

**Estado:** en curso. Esta fase consolida evidencia reproducible; no amplía por sí sola la compatibilidad del lenguaje.

## Inventario inicial

| Subfase | Estado inicial verificado | Evidencia disponible / trabajo pendiente |
|---|---|---|
| 1. Matriz por componente | Implementación inicial | [`testing.md`](testing.md#matriz-de-trazabilidad-de-capacidades-anunciadas) enlaza áreas anunciadas con funciones de prueba. Una prueba del recolector valida que los identificadores documentados existen; todavía requiere revisión humana del alcance de cada evidencia. |
| 2. Regresiones | Implementada en el corpus existente | Los defectos corregidos tienen pruebas en el crate afectado; fuzz crashes futuros deben añadirse como seed y regresión determinista. |
| 3. Diferencial contra Rust | Parcial | Seis casos positivos compilan y ejecutan pares `.ox`/Rust para iteradores/colecciones, `struct`/`impl`/método/`match`, propagación `Result`/`?`, patrones de enum, préstamo mutable/iteración y polling de async; un caso negativo compara `E0308` y su mapeo al `.ox`. Es una muestra acotada. |
| 4. Ida y vuelta | Implementada para subconjuntos probados | CLI verifica `.ox → .rs → .ox → .rs` y roundtrip de árboles mixtos; no equivale a roundtrip universal. |
| 5. Fuzzing | Campaña extendida inicial aprobada | Cinco targets con corpus semilla y smoke CI de 1.000 casos; se ejecutó localmente una campaña de 10.000 casos por target y se generó el informe reproducible. Esto sigue siendo una campaña corta, no fuzzing prolongado. |
| 6. Límites y recuperación | Parcial | Hay límites de tamaños/anidamiento, Unicode, spans y framing. El LSP anuncia sincronización incremental y tiene pruebas de ediciones en secuencia con UTF-16, versiones obsoletas, rangos inválidos y recuperación. Semantic tokens admite cancelación cooperativa mediante `$/cancelRequest`; los diagnósticos de edición permanecen síncronos. |
| 7. Proyectos reutilizables | Parcial | `reference-regressions` automatiza en CI interoperabilidad con `std`, CRUD concurrente y overflow, los árboles Rust de módulos, parser de la CLI exportada y su línea base Rust, API SQLite Óxido/Rust secuencial, y persistencia del árbol exportado. Los manifiestos Rust de referencias también se comprueban con rustfmt. El benchmark comparativo queda automatizado semanalmente y puede iniciarse desde `workflow_dispatch`; esta implementación no ejecutó ni reportó una nueva medición. Las interacciones de terminal y referencias listadas como manuales en `tests/README.md` siguen pendientes. |
| 8. Informes reproducibles | Integrado en CI | `tools/validation-report` registra commit/entorno/estados/duración y escribe Markdown; `validation-report` ejecuta el recolector e instala las herramientas requeridas y conserva `target/validation-report.md` como artefacto de Actions por 30 días, incluso si fallan suites después de generar el archivo. La campaña de 10.000 por target se archivó en [`reports/fase-10/2026-10-26-10000-por-target.md`](reports/fase-10/2026-10-26-10000-por-target.md). No se inventan métricas de rendimiento. |
| 9. CI | En progreso | Además de fmt, Clippy, tests, cargo-deny y smoke fuzz, la CI incluye informe descargable y regresiones de referencia SQLite. La comparación de rendimiento queda como workflow optativo, separada de la verificación funcional. Este cambio aún requiere su propio resultado remoto verde. |

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
| Límites | Solo dos fixtures positivos; no cubre equivalencia semántica general ni diagnostica pares de errores equivalentes |

## Iteración de diagnóstico diferencial

| Dato | Valor |
|---|---|
| Base | `654938990534c691a481295e257d2b69c00110be` |
| Caso | `differential_type_errors_match_rustc_and_map_back_to_the_ox_line`: `.ox` y Rust directo rechazan `E0308`; el diagnóstico de Óxido mapea a la línea 3 fuente |
| Verificación local | Prueba dirigida aprobada; suite completa y CI remota pendientes |

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
| Commit probado | `8311436f565af6fb1915820e8f617c338e487950` |
| Decoder | El fuzzer consume bytes del framing `Content-Length` y despacha payloads válidos; máximo 8 KiB por línea de cabecera y 4 MiB por frame; longitudes duplicadas se rechazan |
| Regresiones de transporte | Frame válido, EOF, Content-Length ausente/duplicado, cabecera demasiado larga, frame sobredimensionado y cuerpo truncado |
| Verificación local | `cargo fmt --all -- --check`, Clippy y `cargo test --workspace --locked`: 138 pruebas aprobadas, 0 fallidas; fuzz manifiesto chequeado con `--locked` |
| Smoke LSP | `cargo +nightly fuzz run lsp -- -runs=1000 -max_len=8192`: 1.000 ejecuciones, sin crash |
| CI remota | Aprobada para este commit: `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages en push y pull request |
| Límites | La cancelación LSP y la recuperación incremental siguen sin implementar/probar |

## Criterio de cierre

La fase seguirá abierta hasta que las características publicadas en la matriz tengan tests identificables, las limitaciones y campañas no ejecutadas figuren como pendientes, los diferenciales/roundtrips relevantes estén automatizados y el job de CI de fuzzing pase. No se exige demostrar exhaustivamente toda la gramática de Rust ni trasladar a este laboratorio la aceptación funcional de crates, macros, runtimes o `unsafe`; esa decisión corresponde a la Fase 8 según ADR-0007.

## Iteración de diferenciales, sincronización LSP y campaña extendida

| Dato | Valor |
|---|---|
| Base de Git para el informe local | `35bfd6c33c66ebfdcc773e897e2fad7741c0db4d`; el árbol estaba modificado durante la ejecución |
| Diferencial nuevo | `differential_result_propagation_matches_native_rust`: compila y ejecuta `.ox` y Rust con `Result`, `?`, ramas y `match`; stdout idéntico |
| LSP | El servidor anuncia sincronización incremental; aplica rangos UTF-16 en orden, rechaza rangos inválidos sin cambio parcial e ignora versiones obsoletas. Semantic tokens corre como trabajo cancelable y `$/cancelRequest` responde `-32800`; los diagnósticos de edición siguen síncronos. |
| Proyectos de referencia | El índice central ahora anota modo, manifiestos/lockfiles, dependencias, instrucción/resultado y si la evidencia corre en CI o manualmente. Se corrigió el modo del proyecto de módulos a llaves. |
| Verificación local | [`Informe integral archivado`](reports/fase-10/2026-10-26-10000-por-target.md): `cargo test --workspace --locked` aprobó 147 pruebas; 5 pruebas del recolector; fmt, Clippy, cargo-deny, build del portal y cinco harnesses aprobaron. Cada target fuzz completó 10.000 iteraciones con `-max_len=8192`, sin crash. |
| Rendimiento | No medido por esta campaña |
| CI remota | Aprobada en PR #27 (`b4994b0`): `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages en ambos workflows |
| Límites | La cancelación solo se aplica a semantic tokens; semantic tokens siguen siendo heurísticos. Los proyectos que levantan servicios y benchmarks continúan como procedimientos manuales; el índice no equivale a automatizarlos. La campaña 10k/target no acredita ausencia de defectos. |

## Iteración de corpus diferencial y referencias CI

| Dato | Estado de esta iteración |
|---|---|
| Diferenciales añadidos | `differential_patterns_borrows_and_async_match_native_rust` ejecuta tres pares `.ox`/Rust: patrones de enum, préstamo mutable con iteración, y función async con polling de `Future`. Se comparan procesos y stdout exacto. El corpus diferencial queda en seis pares positivos y una comparación negativa de `E0308`; sigue siendo cobertura de casos, no equivalencia universal. |
| Informe en CI | Job `validation-report`: corre el recolector con Rust 1.99.1, nightly fijado, cargo-deny, cargo-fuzz y dependencias del portal; sube el Markdown como artefacto asociado al commit durante 30 días. Si el recolector produce el archivo y luego falla, el paso de subida usa `always()`. |
| Referencias funcionales | Job `reference-regressions`: `reference-runner` ejecuta interoperabilidad con `std`, CRUD concurrente y overflow; regenera el Rust exportado, comprueba formato y determinismo, ejecuta los tests de parser de la CLI exportada y Rust, descarga dependencias con Cargo, ejecuta secuencialmente los árboles Rust y servidores Óxido/Rust sin cargas medidas, y valida persistencia/reinicios del proyecto exportado. |
| Benchmark | Se ejecuta automáticamente cada lunes en GitHub Actions y también puede iniciarse con `workflow_dispatch` (`ejecutar_benchmark`). Conserva el log del benchmark release de cinco rondas. No fue ejecutado en esta iteración; **rendimiento no medido**. |
| Verificación local | Aprobados `cargo fmt --all -- --check`, Clippy y tests del workspace (148), fmt/Clippy/tests del recolector (5), `cargo deny check`, cinco fuzz smoke de 1.000 iteraciones y build de website. `reference-runner` pasó `std`, 3.200 altas totales en olas de hasta 100 y overflow; pruebas HTTP SQLite secuenciales y persistencia exportada tras reinicios; ambos árboles Rust de módulos; y los tests del parser de terminal Rust/exportado. |
| CI remota | Pendiente para esta rama; el job de informe, las regresiones de referencia y el workflow completo deben pasar en Actions antes de afirmar que están integrados. |
| Límites | Las olas del CRUD funcional quedan limitadas a 100 solicitudes simultáneas; los lotes de 500 y 1.000 comprueban volumen e IDs en varias olas. El benchmark depende del host de GitHub y es exploratorio; sus números no se usan como criterio de compatibilidad ni rendimiento de producción. Las referencias restantes se enumeran en `tests/README.md`. |

## Iteración de trazabilidad y referencias

| Dato | Valor |
|---|---|
| Base | Merge de informes reproducibles: `2e90a9e08f00ecc75a9b4b3dca5c4d6c871aac52` |
| Cambios | Matriz de capacidades a pruebas con identificadores; prueba del recolector que detecta referencias a funciones eliminadas/renombradas; índice de proyectos de referencia y distinción automática/manual |
| Verificación local | Reporte `tools/validation-report`: fmt, Clippy y 139 pruebas del workspace aprobadas; cargo-deny aprobado; compilación y smoke de 1.000 iteraciones en los cinco targets fuzz aprobados; build del portal aprobado. La suite independiente del recolector aprobó 4 pruebas, incluida la nueva comprobación de trazabilidad. |
| CI remota | Aprobada en PR #24 (`230e705`): `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages en los workflows de push y PR |
| Límites | El índice no convierte ejecuciones de servicios ni benchmarks en pruebas de CI; cancelación LSP, recuperación incremental sistemática, diferenciales más amplios y campañas prolongadas continúan pendientes. Rendimiento no medido por el informe. |

## Iteración de recuperación LSP

| Dato | Valor |
|---|---|
| Base | Merge de trazabilidad: `7adf63dc70737393703e538074e0ec66b9dce68b` |
| Caso | `recovers_diagnostics_after_a_full_document_change`: abre un `.ox` con error de indentación, envía `didChange` de sincronización completa y exige publicación de diagnósticos vacía |
| Verificación local | `cargo fmt --all -- --check`, `cargo test -p oxido-lsp --locked`, Clippy de `oxido-lsp` y el informe integral aprobados; 140 pruebas del workspace, 4 del recolector y 8 de `oxido-lsp`; fuzz-smoke 1.000 por target, cargo-deny y build del portal aprobados |
| CI remota | Aprobada en PR #25 (`84f071d`): `rust`, `dependencies`, `fuzz-smoke` y Cloudflare Pages en los workflows de push y PR |
| Límites | Comprueba recuperación ante reemplazo completo, no cambios incrementales por rango; el dispatcher síncrono no implementa cancelación de solicitudes. |
