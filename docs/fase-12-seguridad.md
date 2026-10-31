# Fase 12 — Seguridad y calidad avanzada

**Estado:** en curso. Las subfases **12.1 a 12.7** están completadas en su alcance documentado. Las subfases 12.8–12.11 siguen abiertas; su cierre requiere la evidencia indicada en el plan de corrección.

**Fuentes:** [`plan` §16](../plan) (modelo de referencia), [`ARCHITECTURE_RULES.md`](../ARCHITECTURE_RULES.md), [`SECURITY.md`](../SECURITY.md), [`docs/politica-dependencias.md`](politica-dependencias.md) y el código de `crates/`. Este informe describe el estado verificado; no declara ausencia de riesgos.

---

## 12.1. Modelo de amenazas

### Activos

1. La máquina del usuario y su workspace: ningún `.ox`, configuración o dependencia debe ejecutar código sin autorización explícita.
2. La integridad del análisis: diagnósticos, navegación y renombrado no deben aplicarse sobre correspondencias dudosas.
3. Los datos del workspace: el índice y la caché no deben escribirse dentro del proyecto ni filtrar rutas absolutas.
4. La cadena de suministro: dependencias, `Cargo.lock`, acciones de CI y artefactos publicados.

### Superficies y controles

| Superficie | Amenaza | Control verificado | Evidencia | Riesgo residual |
|---|---|---|---|---|
| `.ox` | Entrada desmesurada o malformada que agota memoria/CPU o provoca pánico | Límites de 4 MiB, 1.000.000 de tokens, 256 grupos, 128 niveles de indentación y 1.000 diagnósticos por fuente; `forbid(unsafe_code)`; fuzzing del lexer y del parser | `lexer.rs` (`MAX_SOURCE_BYTES`, `MAX_TOKENS`, `MAX_GROUP_DEPTH`, `MAX_INDENT_DEPTH`, `MAX_DIAGNOSTICS`), `parser.rs` (`MAX_BLOCK_DEPTH`), `fuzz/fuzz_targets/{lexer,parser}.rs` | Una entrada válida de 4 MiB puede consumir el presupuesto completo de 10 s; la medición del peor caso pertenece a 12.8 |
| `.ox` | Suplantación visual con controles bidireccionales, alfabetos mezclados o formas de ancho completo | `OX1002` (error) para controles bidireccionales; `OX1003` (warning) para identificadores con alfabetos confundibles, armenio o formas/dígitos de ancho completo | `lexer.rs` (`is_bidi_control`, `has_confusable_identifier`), prueba host `extension.test.js` | La advertencia no bloquea; depende del editor para mostrarla y no cubre todos los homoglifos Unicode |
| `.ox` | Análisis sin fin por construcción patológica | Plazo cooperativo de 10 s con `OX6001`; los tokens parciales no se parsean | `ANALYSIS_TIME_BUDGET`, `lex_with_deadline`, `parse_with_deadline`; pruebas `stops_lexing_when_the_deadline_expires_without_panicking` y `stops_parsing_when_the_deadline_expires_without_panicking` | El plazo se comprueba por lotes; un único token gigante (comentario o literal) puede agotar el presupuesto restante antes de la siguiente comprobación |
| `.ox` | Cascada de diagnósticos que agota memoria o inunda el informe | Tope de 1.000 diagnósticos por fuente más un resumen `OX6001` con los omitidos; mismo tope para los diagnósticos de Rust | `MAX_DIAGNOSTICS` en `lexer.rs` y `parser.rs`; truncado en `parse_ox` (`oxido-compiler`) | El resumen cuenta los omitidos, pero no los clasifica por gravedad |
| `oxido.toml` | Configuración manipulada que provoca comandos, rutas fuera del proyecto o escalada | Formato declarativo sin comandos; rutas canonicalizadas y confinadas a la raíz; límites de tamaño de fuente; symlinks no seguidos en el índice | `config.rs`, `application/modules.rs`, `crates/oxido-lsp/src/index.rs` | La canonicalización depende del sistema de archivos; una ruta puede cambiar entre la comprobación y el uso |
| Caché LSP | Lectura de la caché por otros usuarios o escritura dentro del workspace | Caché fuera de la raíz; escritura temporal + `rename`; permisos `0700` en el directorio y `0600` en el archivo en Unix | `crates/oxido-lsp/src/index.rs` (`restrict_permissions`), prueba `restricts_cache_permissions_to_the_current_user` | En Windows los permisos dependen de las ACL heredadas; no se aplica un modo POSIX |
| `unsafe` analizado | Uso de `unsafe` en código Rust o alias `inseguro` en `.ox` | `#![forbid(unsafe_code)]` en los seis crates y el binario del LSP; `oxido auditar-unsafe` y `oxido auditar-proyecto` con `--rechazar` | `crates/*/src/lib.rs`, `crates/oxido-lsp/src/main.rs`, `crates/oxido-cli/src/main.rs`, pruebas de `tests/cli.rs` | La auditoría es léxica: no ve expansiones macro ni código generado; `--rechazar` solo cuenta tokens detectados |
| Dependencias y macros | `build.rs` o macros procedurales maliciosas que ejecutan código al compilar | Compilación solo con autorización explícita (`OX7002`), `--locked --offline` en las rutas documentadas y Workspace Trust en el editor | `crates/oxido-compiler/src/application/cargo.rs`, `docs/fase-8-integracion-rust.md` | La compilación autorizada ejecuta código de terceros; no hay sandbox de sistema operativo |
| LSP | Tramas malformadas, cabeceras gigantes o cuerpos enormes | Cuerpo máximo 4 MiB, cabecera 8 KiB, rechazo de longitud duplicada/ausente y truncamientos; pruebas deterministas y fuzz | `read_frame`, `validate_frame_length`, pruebas de framing | El cliente stdio se considera del mismo usuario; no hay autenticación ni aislamiento de proceso |
| LSP | Ráfaga de `didChange` o de solicitudes que sature el servidor | 64 documentos, 4 MiB por documento, 16 MiB totales, 4 solicitudes semánticas, 1 comprobación Cargo, 1 tarea de diagnóstico y cola de eventos de 8; cancelación de trabajo obsoleto | `crates/oxido-lsp/src/lib.rs`; prueba `bounds_a_burst_of_changes_to_one_queued_diagnostic` | La política descarta trabajo, no lo encola; el usuario ve el último estado, no cada paso intermedio |
| LSP | Escritura de caché dentro del workspace o fuga de rutas en errores | Caché versionada fuera de la raíz, escritura temporal + `rename` atómico, permisos `0700`/`0600` en Unix y errores sin rutas absolutas | `crates/oxido-lsp/src/index.rs`; pruebas de persistencia, invalidación y permisos; host `run-host.js` | En sistemas no Unix los permisos dependen de las ACL del sistema operativo |
| LSP | Ejecución implícita de Cargo o `rust-analyzer` en un workspace no confiable | Restricted Mode no inicia `rust-analyzer` ni permite `oxido/check`; solo índice y ámbitos locales | `run-host.js` (casos sin `--disable-workspace-trust`), pruebas host de rename y Cargo | Si el usuario concede confianza, el análisis puede ejecutar build scripts y macros |
| Procesos hijos | Inyección de comandos o procesos huérfanos | `Command` con argumentos separados, nunca shell; `rustfmt` con plazo de 10 s; Cargo con timeout y cancelación que mata el árbol | `oxido-rust-backend/src/lib.rs` (`TIME_LIMIT`), `application/cargo.rs` | Depende de las garantías del sistema operativo al matar el árbol |
| CI y artefactos | Fuga de secretos o rutas absolutas; dependencias vulnerables | Informes con nombres de pruebas y sin logs completos; `cargo-deny` y `cargo-audit` en CI; `--locked` | `.github/workflows/ci.yml`, `deny.toml`, `docs/politica-dependencias.md` | `cargo-audit` depende de la base RustSec y no detecta vulnerabilidades desconocidas; SBOM/firma pertenecen a 12.9 |
| Playground | Ejecución de código ajeno | **No existe**; si se aprueba, exige aislamiento del sistema operativo, red deshabilitada y límites de CPU, memoria, procesos y tiempo | `plan` §16.1; no hay directorio `playground/` | Subfase 12.11 condicionada a la aprobación del producto |

### Fronteras de confianza

- **Contenido no confiable:** `.ox`, `oxido.toml`, `Cargo.toml`/`Cargo.lock`, dependencias, macros y parámetros LSP.
- **Confianza explícita:** la habilita Workspace Trust de VS Code y la autorización de compilación de dependencias. Sin ella no se inicia Cargo ni `rust-analyzer`.
- **Confianza implícita:** el cliente LSP que habla por stdio y el usuario que ejecuta la CLI. No se modela un atacante con control del proceso del editor.

---

## 12.2. Límites aplicados

### Matriz de límites

| Recurso | Límite | Punto de aplicación | Diagnóstico | Prueba |
|---|---|---|---|---|
| Tamaño de fuente | 4 MiB | Lexer | `OX6001` | `enforces_source_and_group_limits_with_diagnostics` |
| Tokens | 1.000.000 (el valor exacto se acepta) | Lexer | `OX6001` | `enforces_the_token_limit_without_rejecting_the_exact_boundary` |
| Anidamiento de grupos | 256 | Lexer | `OX6001` | `enforces_source_and_group_limits_with_diagnostics` |
| Anidamiento de indentación | 128 | Lexer | `OX6001` | `enforces_source_and_group_limits_with_diagnostics` |
| Anidamiento de bloques y control | 128 | Parser | `OX6001` | `parser_limits_recursive_block_depth_with_a_diagnostic`, `parser_limits_recursive_else_if_depth_with_a_diagnostic` |
| Tiempo de análisis | 10 s | Lexer y parser, plazo cooperativo | `OX6001` («el análisis superó el límite de tiempo») | `stops_lexing_when_the_deadline_expires_without_panicking`, `stops_parsing_when_the_deadline_expires_without_panicking` |
| Trama LSP | 4 MiB de cuerpo, 8 KiB por cabecera | Decoder stdio | Error de framing | `rejects_duplicate_missing_oversized_and_truncated_frames`, `rejects_a_header_line_over_the_limit_without_reading_the_body` |
| Documentos abiertos | 64; 4 MiB por documento; 16 MiB totales | Estado LSP | `OX6002`/`OX6003` según el caso | `enforces_open_document_and_total_memory_limits` |
| Solicitudes concurrentes | 4 semánticas, 1 Cargo | Bucle LSP | Respuesta de error/cancelación | Pruebas de concurrencia y cancelación |
| Diagnósticos en cola | 1 tarea en vuelo; ráfagas descartadas y el documento queda sucio | `dispatch_diagnostic` | No publica trabajo obsoleto | `bounds_a_burst_of_changes_to_one_queued_diagnostic` |
| Diagnósticos por fuente | 1.000 más un resumen con los omitidos | Lexer, parser y `parse_ox` | `OX6001` | `caps_diagnostics_and_reports_the_omitted_count`, `caps_parser_diagnostics_and_reports_the_omitted_count` |
| Permisos de caché | Directorio `0700` y archivo `0600` en Unix | Índice LSP | Error de escritura; no se publica la caché | `restricts_cache_permissions_to_the_current_user` |
| Eventos LSP | Cola acotada de 8 | Canal de eventos | Descarte controlado | `canceled_semantic_token_work_stops_and_returns_request_cancelled` |
| Índice y caché | 256 archivos, 2048 entradas de escaneo, 4 MiB por archivo, 16 MiB de caché | Índice LSP | Sin escritura si excede | `persists_and_reuses_symbols_within_the_cache_directory` |
| `rustfmt` | 10 s | Backend Rust | `OX4001` si falla | Pruebas del backend |
| Cargo | Timeout y cancelación que mata el árbol | `application/cargo.rs` | Resultado cancelado | Host de cancelación |

### Tiempo de análisis

El plazo es **cooperativo y aditivo**: `lex` y `parse` conservan su firma sin límite de tiempo, y `lex_with_deadline`/`parse_with_deadline` aceptan `Option<Instant>`. El compilador (`parse_ox`, y por tanto `generate_ox`) usa `analysis_deadline()`; el LSP lo aplica en `lex_analyzed` y la CLI en sus lecturas de `.ox`.

- El lexer comprueba el reloj cada 4096 pasos del bucle principal y al entrar, de modo que un plazo ya vencido no produce ningún token.
- El parser comprueba por item y por sentencia, y marca `timed_out` para que los bucles anidados terminen sin cascada de errores.
- Si el lexer se detiene, `parse_with_deadline` devuelve sus diagnósticos sin parsear tokens parciales.
- Las operaciones del LSP que recorren varias fuentes (referencias de workspace y símbolos del índice) comparten además un plazo por operación y se detienen al vencer, de modo que el coste agregado no crece con el número de archivos.
- `ANALYSIS_TIME_BUDGET` es una constante pública documentada; cambiarla exige medir el peor caso legítimo en 12.8.

### Frecuencia

No hay token-bucket ni límite por cliente. La frecuencia se controla con **trabajo acotado y coalescencia**:

- Solo hay una tarea de diagnóstico en vuelo. Cada `didChange` cancela la anterior; si el canal está lleno, la tarea se descarta y el URI permanece sucio para reintentarse al liberarse el worker.
- Las solicitudes semánticas se limitan a cuatro y Cargo a una; el resto recibe cancelación o error explícito.
- La cola de eventos tiene capacidad 8 y el decoder rechaza tramas fuera de límite antes de asignar memoria proporcional al cuerpo.

Esto protege al servidor de ráfagas sin prometer una cuota por segundo ni descartar silenciosamente el último estado del documento.

### Lo que estos límites no cubren

- No sustituyen un sandbox: la compilación autorizada puede ejecutar `build.rs` y macros.
- No limitan la memoria total del proceso más allá de los topes por documento, índice y cola.
- No garantizan que un plazo de 10 s sea suficiente para toda entrada válida; la medición Linux está en el [informe de benchmark](reports/fase-12/2026-10-30-benchmark-linux.md) y Windows/macOS se miden en CI.
- No hay playground ni aislamiento de sistema operativo.

---

## Resultados de auditoría (12.1–12.9)

| Comprobación | Resultado |
|---|---|
| Límites de tamaño, tokens, grupos, indentación y bloques | Verificados en código y pruebas existentes; sin cambios en 12.1–12.2 |
| Plazo de análisis en lexer y parser | Implementado y probado con plazos vencidos y futuros |
| Presupuesto aplicado en compilador, LSP y CLI | Verificado en `parse_ox`, `lex_analyzed` y las lecturas de la CLI |
| Coalescencia de diagnósticos bajo ráfaga | Probada con 63 cambios y una cola de capacidad 1 |
| `#![forbid(unsafe_code)]` en los seis crates y binarios | Verificado en 12.3 |
| Política `--rechazar` | Implementada en `auditar-unsafe` y `auditar-proyecto`, con pruebas de rechazo |
| Detección bidi y confundibles | Verificada (`OX1002`/`OX1003`); ampliada a armenio y ancho completo en 12.4 |
| Rutas, symlinks y permisos | Auditados en 12.5; el índice no sigue symlinks y la caché usa `0700`/`0600` en Unix |
| Procesos hijos | Sin shell; argumentos separados; cancelación de Cargo mata el árbol |
| `cargo-deny` | Configurado y ejecutado en CI |
| `cargo-audit` | Integrado con versión fijada 0.22.2; sin vulnerabilidades conocidas en la auditoría local (12.6) |
| Fuzzing y pánicos | Cinco targets, smoke de 1000 casos por push y 10 000 semanales; revisión estática de pánicos completada (12.7) |
| Tope de diagnósticos | 1.000 por fuente más resumen `OX6001`; el mismo tope se aplica a los diagnósticos de Rust antes de combinarlos (12.7) |
| Benchmarks y memoria | Linux medido con objetivos y RSS real; jobs de Windows/macOS preparados para 12.8 |
| Builds reproducibles | Dos builds release con `SOURCE_DATE_EPOCH=0` y remap de rutas producen el mismo SHA-256 en Linux (12.9) |
| SBOM | CycloneDX 1.5 por crate, sin rutas absolutas del workspace; artefacto de CI (12.9) |

### 12.3. `unsafe`

- Los seis crates (`oxido-language`, `oxido-rust-backend`, `oxido-cargo`, `oxido-compiler`, `oxido-cli`, `oxido-lsp`) y el binario `oxido-lsp/src/main.rs` declaran `#![forbid(unsafe_code)]`; el workspace además lo configura en `[workspace.lints.rust]`.
- `oxido auditar-unsafe <archivo> [--rechazar]` detecta el token `unsafe` y su alias `inseguro` sin compilar y rechaza el archivo si se pide.
- `oxido auditar-proyecto <directorio> [--rechazar]` recorre el proyecto sin seguir symlinks, omite directorios de build, limita las entradas y devuelve error si encuentra usos cuando se aplica la política.
- La auditoría es léxica: no ve expansiones de macros ni código generado. El informe del comando lo declara explícitamente.

### 12.4. Unicode

- Los controles bidireccionales (`U+061C`, `U+200E..=U+200F`, `U+202A..=U+202E`, `U+2066..=U+2069`) se rechazan con `OX1002`.
- `OX1003` advierte cuando un identificador mezcla alfabetos latinos con griego, cirílico o armenio, o cuando contiene formas/dígitos de ancho completo (`U+FF10..=U+FF19`, `U+FF21..=U+FF3A`, `U+FF41..=U+FF5A`).
- La advertencia no bloquea la compilación. No se pretende cubrir todos los homoglifos de Unicode; el control reduce el riesgo de suplantación visual, no lo elimina.

### 12.5. Rutas, symlinks, permisos y procesos

- El índice del LSP usa `symlink_metadata` y omite enlaces simbólicos; las rutas se canonicalizan y se comprueba que permanezcan dentro de la raíz del workspace.
- La caché se escribe fuera de la raíz, mediante archivo temporal y `rename` atómico. En Unix el directorio recibe modo `0700` y el archivo `0600`; si falla la restricción de permisos, se elimina el temporal y no se publica la caché.
- El workspace temporal de compilación rechaza que su raíz sea un symlink y verifica que el directorio canónico siga dentro de la raíz temporal.
- Los procesos se invocan con `Command` y argumentos separados, nunca mediante shell. `rustfmt` tiene plazo de 10 s y Cargo tiene timeout con cancelación que mata el árbol.
- Los errores del índice no incluyen rutas absolutas; las pruebas comprueban que la caché no aparece dentro del workspace.

### 12.6. Cadena de suministro

- `cargo audit` 0.22.2 se ejecuta en local y en los jobs `dependencies` y `validation-report` de CI con versión fijada. La auditoría local sobre `Cargo.lock` (29 dependencias) no reportó vulnerabilidades conocidas.
- `cargo deny check` aplica licencias `Apache-2.0`, `MIT` y `Unicode-3.0`, rechaza registros y fuentes Git desconocidas y reporta avisos de dependencias sin mantenimiento.
- La política de licencias, excepciones, cadencia y límites está en [`docs/politica-dependencias.md`](politica-dependencias.md).
- El recolector de validación ejecuta `cargo audit` como comprobación opcional; si la herramienta no está instalada, el informe la marca como **No evaluada**, y en CI se instala para que un fallo rompa el job.

### 12.7. Fuzzing, pánicos y memoria/CPU

- Cinco targets de `cargo-fuzz` (`lexer`, `parser`, `config`, `backend`, `lsp`), con smoke de 1000 casos por push y 10 000 semanales. Un smoke no demuestra ausencia de fallos.
- Revisión estática de rutas de producción: no hay `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` ni `assert!`/`debug_assert!` en el código de biblioteca fuera de módulos `#[cfg(test)]`. Las únicas coincidencias son nombres de macros en las tablas de palabras clave de Rust. Las pruebas sí usan `unwrap`/`expect` para fallar con contexto.
- Los límites de memoria y CPU son los de la matriz: 4 MiB por fuente, 1.000.000 de tokens, 256 grupos, 128 niveles, 1.000 diagnósticos, 64 documentos de 4 MiB con 16 MiB totales, 256 archivos indexados y 16 MiB de caché.
- El plazo cooperativo de 10 s acota el tiempo de análisis, pero no el consumo total del proceso ni el peor caso de una entrada válida de 4 MiB; la medición Linux está en 12.8.
- El desbordamiento de pila por anidamiento queda acotado por los límites del lexer y del parser; el fuzzing cubre entradas malformadas y truncadas.

### 12.8. Benchmarks de análisis, generación, memoria y LSP

- Arnés propio `tools/benchmark` (`oxido-benchmark`), con 13 escenarios y un proceso hijo por escenario para aislar el pico de RSS. `--check` falla por P95, RSS, plazo vencido o resultado funcional vacío.
- Medición Linux `x86_64`, 12 CPUs y Rust 1.99.1, documentada en el [informe de benchmark](reports/fase-12/2026-10-30-benchmark-linux.md).
- Entrada grande válida de 4 MiB: lexer P95 41,23 ms y 40,2 MiB; parser P95 72,63 ms y 107,4 MiB; generación P95 3.436,55 ms y 1.022,0 MiB; apertura LSP 415,84 ms y 114,3 MiB.
- Escenarios LSP interactivos sobre ~5 KiB: hover, símbolos y referencias con P95 entre 0,63 ms y 1,05 ms en esta máquina.
- Entrada inválida de 4 MiB: 213,05 ms (lexer) y 215,52 ms (parser), con el tope de 1.001 diagnósticos y sin plazo vencido.
- Límite conocido: una única función con ~300.000 sentencias desborda la pila de `rustfmt` y devuelve `OX4001`; el benchmark usa muchas funciones pequeñas para la carga representativa. No toda entrada válida de 4 MiB se genera correctamente.
- Los objetivos se fijaron después de medir, con margen para la variabilidad del runner. No son objetivos de producción.
- La matriz `benchmark-analysis` ejecuta `--check` en Linux, Windows y macOS. Los resultados de Windows/macOS se incorporarán cuando el primer run de CI los produzca; no se declaran medidos todavía.

### 12.9. Builds reproducibles y SBOM

- `tools/verify-reproducible-build.sh` compila dos veces con `SOURCE_DATE_EPOCH=0`, `CARGO_INCREMENTAL=0` y `--remap-path-prefix=<workspace>=/oxido`, y compara SHA-256. En Linux produjo el mismo hash para `oxido-cli` (documentado en [`docs/reproducibilidad.md`](reproducibilidad.md)).
- `tools/generate-sbom.sh` usa `cargo-cyclonedx` 0.5.9, genera CycloneDX 1.5 JSON por paquete, renombra los archivos y elimina la ruta absoluta del workspace. CI publica el artefacto `oxido-sbom-<sha>`.
- Los jobs `reproducible` y `sbom` se añadieron a CI y al workflow de release. El workflow de release no firma, no notariza y no publica; solo empaqueta y adjunta artefactos.
- La reproducibilidad comprobada es local, con el mismo toolchain y la misma máquina. No se ha demostrado entre sistemas operativos ni máquinas distintas.
- Firma, notarización, publicación y credenciales siguen pendientes de autorización explícita.

Comandos ejecutados para esta fase:

```bash
cargo test -p oxido-language --test lexer --locked
cargo test -p oxido-language --test parser --locked
cargo test -p oxido-lsp --locked bounds_a_burst
cargo test -p oxido-cli --test cli --locked audits
cargo test -p oxido-cli --test cli --locked reports_project_risk_surface
cargo audit
cargo deny check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo fmt --manifest-path tools/validation-report/Cargo.toml -- --check
cargo clippy --manifest-path tools/validation-report/Cargo.toml --all-targets --locked -- -D warnings
OXIDO_VALIDATION_FUZZ_RUNS=1000 cargo run --manifest-path tools/validation-report/Cargo.toml --bin oxido-validation-report --locked
cargo run --release --locked -p oxido-benchmark -- --check
tools/verify-reproducible-build.sh oxido-cli oxido release
tools/generate-sbom.sh
```

---

## Plan de corrección

| Prioridad | Acción | Subfase | Estado |
|---|---|---|---|
| Media | Medir el peor caso de 4 MiB y un millón de tokens para justificar el presupuesto de 10 s | 12.8 | Completado en Linux; Windows/macOS en CI |
| Media | Añadir objetivos de P95/RSS y comprobar que el escenario hizo trabajo real | 12.8 | Completado |
| Media | Verificar dos builds release con el mismo SHA-256 y generar SBOM sin rutas absolutas | 12.9 | Completado en Linux |
| Media | Revisar pánicos y límites de memoria/CPU con fuzzing prolongado | 12.7 | Completado en el alcance verificado |
| Media | Añadir `cargo-audit` explícito y documentar la política de licencias y dependencias | 12.6 | Completado |
| Media | Auditar permisos de la caché, symlinks y rutas de escritura del LSP | 12.5 | Completado |
| Baja | Verificar la política `--rechazar` de `oxido auditar-unsafe` y documentar sus límites | 12.3 | Completado |
| Baja | Ampliar la cobertura de caracteres confundibles más allá de alfabetos mezclados | 12.4 | Completado |
| Diferida | SBOM y builds reproducibles | 12.9 | Completado en Linux; matriz Windows/macOS en CI |
| Diferida | Firma, notarización y publicación | 12.9 | Pendiente de credenciales y autorización |
| Condicionada | Sandbox de playground, solo si se aprueba el producto | 12.11 | Pendiente |

---

## Checklist de seguridad

- [x] Contenido no confiable identificado por superficie (`.ox`, configuración, dependencias/macros, LSP).
- [x] Límites de tamaño, tokens, anidamiento y tiempo aplicados con diagnóstico reproducible.
- [x] Frecuencia acotada por coalescencia y colas de capacidad fija, con prueba de ráfaga.
- [x] `unsafe` prohibido en el código interno de la toolchain.
- [x] Controles Unicode bidi y de alfabetos confundibles.
- [x] Rutas canonicalizadas y caché fuera del workspace.
- [x] Procesos invocados sin shell; la cancelación mata el árbol de Cargo.
- [x] Workspace Trust respetado por la extensión y el LSP.
- [x] Auditoría formal de permisos y symlinks cerrada (12.5).
- [x] `cargo-audit` integrado y política de dependencias escrita (12.6).
- [x] Revisión de pánicos y memoria/CPU completada en el alcance verificado (12.7).
- [x] Objetivos de rendimiento medidos en Linux con RSS real (12.8).
- [ ] Objetivos de rendimiento medidos en Windows y macOS: jobs preparados, resultados pendientes del primer run de CI (12.8).
- [x] Build reproducible verificado localmente en Linux (12.9).
- [x] SBOM CycloneDX generado y publicado como artefacto de CI (12.9).
- [ ] Firma, notarización y publicación (12.9): requieren credenciales y autorización.
- [ ] Sandbox de playground, solo si se aprueba (12.11).
