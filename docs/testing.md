# Criterios de aceptación y cobertura de pruebas

Estos criterios describen los contratos de aceptación del subconjunto documentado. Son contratos, no evidencia de que exista una función ni de que se hayan ejecutado las pruebas; el estado real lo determinan la suite y la [matriz de compatibilidad](compatibility.md).

| Área del contrato | Componente responsable |
|---|---|
| Configuración declarativa y descubrimiento de fuentes | `oxido-language` y `oxido-compiler` |
| Tokens, modo, Unicode e indentación | Lexer |
| AST, bloques, controles y recuperación | Parser |
| Generación, semántica `rustc`, mapas e ida/vuelta | `oxido-rust-backend` y `oxido-compiler` |
| Normalización de aliases, colisiones y equivalencia de modos | Frontend, backend y pruebas diferenciales |
| Corpus de conformidad, fuzzing y CI extendido | CI y harness de pruebas |
| Semantic tokens, diagnósticos y Workspace Trust | `oxido-lsp` y extensión de VS Code |

## Configuración y modo

- Config `indentation` y header coincidente: acepta.
- Config `braces` y header coincidente: acepta.
- Config y header contradictorios: `OX0002` en el header.
- Sin configuración ni header: `OX0003`.
- Header después de un item, duplicado o cambio de modo: error.
- Modo llaves con indentation irrelevante: mismo AST.
- Bloque estructural con delimitador del otro modo: `OX2002`.
- `{}` en macro, atributo, cadena y struct literal se conserva.

## Semicolon y bloques

- Tail expression sin `;` genera bloque con ese valor.
- La misma expresión con `;` produce `()`.
- Tipo esperado `i32` acepta sin `;` y rechaza donde `i32` se requiere al agregar `;`.
- Statement de expresión no final recibe terminador Rust necesario.
- `let`, `return`, `break`, `continue` y control de bloque generan Rust válido.
- Nueva línea en grupos balanceados no cierra sentencia.
- Operador de continuación conserva expresión multilineal; escape final `\` se rechaza.
- Tab en prefijo, indentación no múltiplo de cuatro y dedentación a nivel inexistente se rechazan con rango exacto.

## Alias e identificadores

- Cada alias estricto tiene prueba individual alias→keyword Rust.
- Alias coincide con keyword en nombre de variable/función: `OX3001` con sugerencia `r#...`.
- `r#tipo` queda como identificador `tipo`, no se normaliza a `type`.
- `mismo` y `Mismo` mantienen diferencia de caso y correspondencia `self`/`Self`.
- `paquete::` se normaliza a `crate::` solo como keyword de path.
- Keywords Rust 2024 reservadas no se habilitan como sintaxis.
- Literales, comentarios, documentación, identificadores no alias y nombres externos permanecen idénticos.

## Macros, atributos y tokens

- Las aliases estándar publicadas normalizan su nombre; el corpus Cargo/rustc verifica aliases ejecutables y el caso negativo `error_de_compilacion!`.
- Argumentos expresivos de macros estándar allowlisted traducen aliases de expresiones anidadas; nombres Rust canónicos siguen funcionando.
- Macros externas/desconocidas y sus DSLs conservan nombre y argumentos opacos sin reescritura.
- Comentarios ordinarios dentro de macros o atributos opacos producen diagnóstico explícito hasta contar con preservación de ubicación.
- Grupos `()[]{}` anidados se conservan.
- Macros braced no se confunden con bloques del modo.

## Comentarios y formato

- Comentarios `//` y `/* ... */` no se traducen y conservan su texto al exportar/importar fuera de grupos opacos.
- Se comprueban comentarios antes de items, inline, dentro/final de bloques, entre brazos, multilínea y al final del archivo.
- Las líneas añadidas por comentarios se reflejan en el mapa de posiciones `.ox`↔`.rs`.
- La regresión CLI `formats_braced_source_idempotently_without_losing_comments` comprueba comentarios previos, doc, inline, bloque, al final, texto de cadenas, `--comprobar` y dos ejecuciones idénticas en modo llaves.
- `new_creates_a_runnable_project_and_rejects_an_existing_directory` comprueba la plantilla de `oxido nuevo`, la ejecuta mediante `oxido ejecutar` y verifica que un segundo intento no altere el proyecto existente.
- `check_validates_without_binary_and_build_release_keeps_the_executable` demuestra que `oxido check` no genera el ejecutable, que `oxido build --release` lo conserva en `target/oxido/target/release`, y ejecuta ese mismo binario para verificar su salida. También fija `compilar` como alias de `build`.
- `build_imported_workspace_leaves_release_binary_under_project_target` importa un workspace Cargo con su árbol de carpetas, ejecuta `oxido build --release` sobre el target `.ox` y ejecuta el binario persistente generado.
- `workspace_root_commands_and_binary_selection_match_cargo` importa un workspace de varios miembros y binarios, ejecuta `verificar`/`compilar`/`probar` desde su raíz, valida artefactos y pruebas, exige selección ante ambigüedad, prueba `-p`/`--bin`, errores de argumentos y nombres inválidos, y comprueba el esquema JSON y las rutas relativas de diagnósticos. `imported_workspace_root_with_one_binary_runs_without_selection` verifica selección automática del único binario.
- Las pruebas de `crates/oxido-cli/tests/workspace_cargo_process.rs` usan un Cargo controlado para comprobar que `compiler-artifact` se presenta antes de terminar el proceso, que `build-finished` informa el resultado, que `--json` no mezcla texto con stdout, que se conserva el código de fallo y que Ctrl+C termina el grupo de procesos. Las pruebas unitarias de `oxido-cargo` verifican el runner sin timeout y un timeout corto inyectado sin una espera de dos minutos.
- `build_rejects_a_symlinked_target_directory` verifica que la salida de build rechace `target/` como enlace simbólico y no escriba fuera del proyecto.
- `test_command_runs_a_real_rust_test_and_rejects_release_mode` comprueba que `oxido probar` ejecute un `#[test]` real, muestre el nombre y resultado cuando falla, propague el código de error y rechace `--release`.
- `import_project_preserves_a_runnable_tree_and_export_roundtrips_it` importa archivos Rust con un módulo convencional, comprueba sus comentarios, ejecuta el árbol `.ox`, lo exporta y compila/ejecuta el árbol Rust fuera del origen.
- `formats_indented_source_idempotently_without_moving_comments_or_literals` comprueba que `formato` mantenga `sintaxis indentacion`, sea idempotente y preserve comentarios, literales y tokens de macros opacos.
- `formats_indented_source_with_rust_like_spacing_and_canonical_nested_indentation` comprueba sangría canónica de bloques y continuaciones, espaciado Rust-like, genéricos, macros preservadas, `--comprobar` y ejecución del programa formateado.
- `formats_long_rust_expressions_into_wrapped_argument_groups` comprueba el ajuste de listas largas de argumentos/elementos a 100 columnas y la idempotencia del resultado.
- `formatter_conformance_matrix_matches_rustfmt_after_rust_export` compara cinco fixtures agrupados por construcciones y confirma que el `.rs` exportado, antes y después de `oxido formato`, ya es exactamente la salida que produce `rustfmt` 1.99.1:

| Fixture | Cobertura concreta |
|---|---|
| `items-y-tipos` | `use` anidado, `derive`, visibilidad, struct genérico, campos, enum, función genérica y `where` |
| `contenedores-y-control` | struct genérico, firma y tipo asociado de trait, `impl` genérico, constante asociada, métodos asociados (incluido receiver mutable), módulo, closure, iteradores, `match` por brazos indentados y expresión tail |
| `macros-comentarios-y-async` | `macro_rules!`, atributo, comentario, raw string multilínea, invocaciones de macros opacas, `async`/`.await`, binding mutable, array, `for`, `while` y `break` |
| `expresiones-y-patrones` | alias de tipo, constante, tupla, referencias, local mutable con tipo, cadenas de métodos, `enumerate`/`fold`, closure con parámetros destructurados, bloque Rust dentro de expresión, `if let`, `return`, `match` y patrones wildcard/binding |
| `control-y-bloques` | `loop`, `if`/`else`, `break` con valor, `continue`, tail expression y ramas con valor |

Cada fixture comprueba el texto `.ox` esperado, vuelve a pasar `oxido formato --comprobar` y exporta antes/después. La prueba conserva el `.rs` emitido, lo procesa con `rustfmt` del toolchain 1.99.1 (`--edition 2024`) y exige igualdad carácter por carácter antes y después. Después `rustc` compila ambos exports como biblioteca; el fixture de macros también compila como ejecutable y comprueba la salida exacta `6\n`. La prueba de formato con llaves comprueba además `rustfmt --check` con comentarios ordinarios y doc. Así se verifica la salida final de Rust para los fixtures enumerados, no una equivalencia semántica universal. Como `.ox` tiene palabras clave y delimitación distintas, su texto no puede ser carácter por carácter igual al `.rs`. La prueba necesita `rustfmt` y `rustc` 1.99.1, instalados en CI.

### Revisión de cobertura estructural admitida

El siguiente cruce cubre cada producción estructural de `docs/grammar.md` con fixtures CLI o de pipeline ejecutables. Los fragmentos `RustExpr`, `RustType`, `RustPattern`, `RustUseTree` y árboles opacos pertenecen a la gramática de Rust: hay representantes diversos y se validan con Syn/rustc, pero no se afirma enumerar todas las expresiones ni todos los tokens posibles de Rust.

| Producción/caso admitido | Evidencia de cobertura |
|---|---|
| `SourceFile`, header de modo, atributo interno y documentación | Cinco fixtures de la matriz (`sintaxis indentacion`, `#![allow]`, `//!`, `///`); regresión de modo llaves y comentarios |
| `FunctionItem`, async, visibilidad, atributos y cabecera Rust | Matriz: funciones genéricas, públicas, `#[inline]`, async/await; regresiones CLI y pipeline para funciones Rust |
| `StructItem`, campos nombrados, genéricos y restricciones | `items-y-tipos` y `contenedores-y-control`; fixtures pipeline para structs Rust |
| `EnumItem` y variantes | `items-y-tipos`; fixtures de parseo/exportación Rust en `crates/oxido-compiler/tests/pipeline.rs` |
| `ImplItem`, impl genérico, trait impl, métodos y receptores | `contenedores-y-control`, incluido método predeterminado, asociados, `&mismo` y `&mutable mismo`; regresión de métodos del pipeline |
| `TraitItem`, declaración/definición de método, tipos y constantes asociados | `contenedores-y-control`; traits Rust en pruebas de pipeline |
| `ModuleItem`, módulo anidado y `UseItem`/árbol de uso | Matriz: `interno` y `usar std::collections::{...}`; regresión de importación de árbol mixto con `#[path]` |
| Items Rust passthrough (`type`, `const`, macro declarativa, atributos) | `expresiones-y-patrones`, `macros-comentarios-y-async`; tabla amplia de items Rust del pipeline |
| Cuerpos con llaves e indentación; cuerpo vacío | `formats_braced_source_idempotently_without_losing_comments`, matriz indentada, fixtures de parser/pipeline |
| `let` tipado/no tipado, mutable, expresiones tail y terminador `;` | `expresiones-y-patrones`, `control-y-bloques` y regresiones del valor tail/semicolon |
| `return`, `break` con valor/etiqueta, `continue` y ramas condicionales | `control-y-bloques`; regresiones de controles Rust en pipeline |
| `loop`, `while`, `for`, `if let`, `match` y brazos con bloque/tail | `control-y-bloques`, `contenedores-y-control`, `expresiones-y-patrones` y `macros-comentarios-y-async` |
| Macros/DSL opacos, grupos anidados, comentarios y literales | `macros-comentarios-y-async`, regresiones de macros externas, `formats_braced_source_idempotently_without_losing_comments` |

La revisión encuentra cobertura para las producciones estructurales enumeradas de la gramática inicial. El alcance restante no es una producción estructural olvidada: es la inmensidad de variantes de expresiones/tipos/patrones y DSL de Rust que Syn preserva o valida. Los documentos de compatibilidad deben seguir presentando cada macro o fragmento externo probado como un caso concreto, no como soporte universal.
- `import_project_rewrites_path_and_keeps_existing_ox_sources_in_a_mixed_tree` importa módulos `.rs`/`.ox` mezclados con `#[path]` relativo, ejecuta el proyecto importado y compila/ejecuta su exportación Rust.
- `import_project_rejects_a_path_attribute_that_escapes_without_creating_output` y `import_project_rejects_a_symlink_output_destination` prueban los rechazos de ruta antes de escribir.
- `cargo_commands_emit_structured_json_for_success_and_source_diagnostics` comprueba el esquema JSON v1 de acciones Cargo de paquete único, stdout exclusivamente JSON, salida saneada y ruta/línea/columna del `.ox` para un error de tipos.

`check`, `build`/`compilar`, `run`/`ejecutar` y `test`/`probar` separan validación, generación de artefactos, ejecución y pruebas. Para archivos `.ox`, la CLI genera un proyecto Cargo en `<raíz>/target/oxido/`, conserva el `.ox` raíz y cada módulo `.ox` (por ejemplo `src/dominio/mod.ox`) junto al Rust generado, y delega el trabajo en Cargo. `--release` selecciona optimizaciones Cargo en `check`, `build` y `run`; `build` guarda el binario en `target/oxido/target/{debug,release}`. `--json` informa `artifactDirectory` para build y `null` en las otras acciones. Un error temprano se representa con `OX0001`; un fallo de Cargo sin diagnóstico mapeable usa `OX5001`. Los comandos ajenos a Cargo no prometen salida estructurada de éxito. El formato indentado normaliza sangría, espaciado y grupos multilínea; preserva grupos opacos de macros, comentarios y literales. Para las construcciones de la matriz, el Rust exportado coincide byte por byte con `rustfmt` 1.99.1. La presentación `.ox` sigue las convenciones de layout documentadas y probadas de Óxido; no pretende ser texto Rust idéntico, pues su sintaxis y palabras clave difieren.

## Fuzzing

`fuzz/` contiene cinco harnesses de `cargo-fuzz`:

| Target | Entrada | Propiedades comprobadas |
|---|---|---|
| `lexer` | Bytes; ignora secuencias que no sean UTF-8 válido | Ejecuta ambos modos, no debe panicar y todo span de token/diagnóstico debe estar en límites UTF-8 de la fuente |
| `parser` | Bytes; ignora secuencias que no sean UTF-8 válido | Ejecuta parser en ambos modos, no debe panicar y spans de diagnósticos/labels/sugerencias deben ser válidos |
| `config` | Bytes; ignora secuencias que no sean UTF-8 válido | Parsea `oxido.toml` declarativo; todo éxito debe respetar Rust 2024 y todo rechazo debe contener diagnóstico |
| `backend` | Bytes; ignora secuencias que no sean UTF-8 válido | Para ambos modos, Rust generado por Syn parseable y rangos del mapa dentro de límites UTF-8 de la fuente |
| `lsp` | Bytes arbitrarios del stream con framing LSP | El decoder de frames y el dispatcher no deben panicar; limita líneas de cabecera a 8 KiB y cuerpos a 4 MiB, rechaza longitud duplicada/ausente y maneja truncamiento |

Los seeds versionados están en `fuzz/corpus/{lexer,parser,config,backend,lsp}`. El target LSP entrega bytes aleatorios al decoder de framing `Content-Length` y, si obtiene un payload completo, al dispatcher JSON-RPC. Las pruebas deterministas cubren frame completo, EOF, cabecera ausente/duplicada, línea demasiado larga, cuerpo truncado y límite de tamaño. Corpus descubierto, crashes, cobertura y binarios se mantienen fuera del repositorio hasta revisarse. El job `fuzz-smoke` ejecuta 1.000 casos con límite de entrada de 8 KiB por target usando cargo-fuzz 0.13.2 y nightly 2026-10-16. El recolector permite campañas locales más largas con `OXIDO_VALIDATION_FUZZ_RUNS`, entre 1.000 y 10.000.000 iteraciones por target; el reporte incluye el número efectivo y el comando. Un smoke no demuestra ausencia de bugs.

`differential_indented_ox_and_equivalent_rust_have_the_same_observable_result`, `differential_indented_impl_and_match_have_the_same_result_as_rust` y `differential_result_propagation_matches_native_rust` comparan tres pares positivos. `differential_patterns_borrows_and_async_match_native_rust` añade cuatro pares ejecutables: enum/patrones, préstamo mutable/iterador, `async`/`Future::poll` y genéricos/trait/closure. Cada par compila y ejecuta ambos programas y compara stdout byte por byte. `differential_type_errors_match_rustc_and_map_back_to_the_ox_line` compara el rechazo `E0308` y exige ubicación en el `.ox`. Son siete pares positivos y uno negativo; no certifican equivalencia semántica general.

## Benchmarks de análisis y LSP

`tools/benchmark` contiene el arnés `oxido-benchmark`. Cada escenario ejecuta un hijo para medir tiempos y otro con una sola ejecución para medir el RSS; el proceso padre muestrea la memoria del segundo y agrega las mediciones.

```sh
cargo run --release --locked -p oxido-benchmark
cargo run --release --locked -p oxido-benchmark -- --check
```

Los escenarios cubren lexer, parser, generación y LSP con entradas típicas (~5 KiB), grandes (4 MiB), inválidas (4 MiB de bytes de control) y consultas LSP de hover, símbolos y referencias. `--check` aplica objetivos de P95 y RSS, comprueba el plazo de 10 s y exige que el escenario haya producido trabajo real (tokens, AST, Rust, mensajes o resultados LSP). Los objetivos se fijaron después de medir en Linux; el job `benchmark-analysis` los repite en Linux, Windows y macOS. Los resultados se conservan como artefactos de CI y en la [preparación empresarial](enterprise-readiness.md).

El benchmark no mide el round-trip de un editor real, la compilación Cargo ni el rendimiento de producción. Un resultado de una máquina no se extrapola a otra.

## Builds reproducibles y SBOM

```sh
tools/verify-reproducible-build.sh oxido-cli oxido release
tools/generate-sbom.sh
```

El primer script compila dos veces en directorios temporales, fija `SOURCE_DATE_EPOCH=0`, elimina la ruta del checkout con `--remap-path-prefix` y compara SHA-256. El segundo genera un CycloneDX 1.5 por paquete con `cargo-cyclonedx` 0.5.9 y elimina la ruta absoluta del workspace. Los detalles, límites y requisitos de firma/publicación están en [`reproducibilidad.md`](reproducibilidad.md).

## Higiene de informes y telemetría

```sh
tools/report-hygiene.sh docs/reports target/sbom target/validation-report.md
tools/audit-telemetry.sh
```

`report-hygiene.sh` falla si un informe contiene rutas absolutas (`/home/`, `/Users/`, `/tmp/`, unidades Windows), credenciales conocidas o volcados de entorno; no imprime el valor coincidente. CI lo ejecuta antes de subir el informe de validación, el SBOM y el log de benchmarks. `audit-telemetry.sh` revisa manifiestos y código propio para rechazar SDK de analítica; no demuestra por sí solo que un binario no abra sockets.

## Sandbox de playground

```sh
cargo test -p oxido-playground --offline
cargo clippy -p oxido-playground --all-targets --offline -- -D warnings
cargo run -p oxido-playground -- --check
cargo build --locked -p oxido-cli -p oxido-playground
tools/playground/smoke.sh
```

`oxido-playground` ejecuta una fuente `.ox` con `bwrap --unshare-all` y `prlimit`. `--check` crea un aislamiento mínimo real; si el kernel no permite namespaces, el servicio no arranca. Las pruebas cubren argumentos de aislamiento, límites de fuente y salida, lectura HTTP acotada y saneado de rutas. `tools/playground/smoke.sh` arranca el servicio y comprueba ejecución, red inaccesible, ruta del host oculta, plazo de pared y rechazo sin `Content-Type`. Se ejecutó tanto en un contenedor Linux anidado como en el job `playground-sandbox` de un runner Ubuntu de GitHub, sin privilegios de contenedor: programa ejecutado, red externa inaccesible desde el sandbox, ruta del host oculta, plazo de pared aplicado y `415` sin `Content-Type`. Estado, método y límites en la [preparación empresarial](enterprise-readiness.md).

## Informe reproducible

Desde la raíz, ejecuta:

```sh
cargo run --manifest-path tools/validation-report/Cargo.toml --bin oxido-validation-report --locked
```

El ejecutor registra commit, rama, estado limpio/modificado, sistema, toolchains, comandos, códigos de salida, duración y totales de tests que Cargo reporta. Incluye sus propios fmt/Clippy/tests y los del workspace, cargo-deny, build/smoke de fuzz y build del sitio. El informe se escribe en `target/validation-report.md`; una suite que falle deja al comando con código distinto de cero. Si una herramienta opcional no está instalada, su estado es **No evaluada**. No copia logs de procesos, por lo que evita filtrar rutas absolutas; solo publica los nombres de las pruebas fallidas como identificadores reproducibles. Este reporte no mide rendimiento y no consulta CI remota desde local. El benchmark release comparativo es una ejecución aparte cuyos resultados se conservan como artefactos de CI. No presentes el reporte de validación como sustituto de resultados remotos.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- El LSP anuncia sincronización incremental y aplica cambios por rangos UTF-16 en secuencia, ignora versiones obsoletas y rechaza rangos inválidos sin alterar el documento. Hay regresiones para recuperación completa/incremental, rangos partidos dentro de un par sustituto y versiones antiguas. El decoder stdio limita cabeceras a 8 KiB y `Content-Length` a 4 MiB; la cola de eventos es acotada. Semantic tokens se construyen desde los tokens del lexer compartido; el índice de declaraciones modela ámbitos locales acotados de parámetros y enlaces `dejar`/`let`/`para`/`for`, usa símbolos de archivos cerrados mediante una caché versionada fuera del workspace y, en workspace confiable, fusiona tokens de `rust-analyzer` con correspondencia token a token exacta. Lo no resuelto conserva la clase léxica. No se declara resolución de tipos universal. `$/cancelRequest` cancela cooperativamente el cálculo asíncrono de semantic tokens y devuelve `-32800`; los diagnósticos de `didOpen`/`didChange` usan `parse_ox` en un worker versionado y descartan resultados obsoletos. El índice local de navegación resuelve funciones, métodos, tipos y módulos con cualificadores, incluye referencias desde la declaración hacia llamadas cualificadas en otros archivos y devuelve todas las candidatas ante nombres repetidos ambiguos. `prepareRename`/`rename` delegan en `rust-analyzer` en workspace confiable y solo aplican ediciones con correspondencia exacta al `.ox`; el fallback local renombra un enlace de su ámbito o un símbolo global único y rechaza ambigüedad, nombres reservados e identificadores inválidos. La caché del índice se versiona, se invalida por fecha/tamaño/hash y se escribe de forma atómica fuera del workspace. `textDocument/formatting` indentado reutiliza el formateador de `oxido formato`. `oxido/check` exige confianza, usa el `oxido.toml` y el `Cargo.lock` del proyecto y agrupa cada diagnóstico de Cargo por el `.ox` de origen; el bucle LSP la despacha a un worker con una sola comprobación concurrente, de modo que otras solicitudes siguen respondiendo. `$/cancelRequest` marca también esa comprobación y mata el árbol de procesos de Cargo; el resultado cancelado no publica diagnósticos. La prueba host comprueba el `E0308` en `util_error.ox`, la navegación/formato en Restricted Mode, el rechazo de Cargo sin confianza, la respuesta/cancelación durante Cargo, tipos concretos en semantic tokens, completado, inlay hints, símbolos, referencias, edición incremental, documento incompleto y tres solicitudes concurrentes de tokens. Los benchmarks de latencia y la matriz Windows/macOS siguen pendientes.

## Interoperabilidad de ida y vuelta

- La importación por defecto conserva el archivo Rust tal cual, añadiendo solo la cabecera `sintaxis llaves`. `import_is_literal_by_default_and_translates_only_with_espanol` y `imports_literally_without_translating_rust_keywords` exigen esa copia exacta.
- Un fixture del subconjunto reversible hace `.ox → .rs → .ox → .rs` y conserva comentarios ordinarios, documentación y semántica cuando se usa `--espanol`.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio documental

La gramática, modos, alias y reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas, y cada decisión tiene un criterio de aceptación asignado a un componente. Que exista el criterio no afirma que esos casos se hayan ejecutado.
## Orquestadores de regresión

Los scripts Python de `prueba/crud_sqlite_rust` y `prueba/crud_sqlite_export_rust` son lanzadores de pruebas de extremo a extremo. Se mantienen en Python porque solo coordinan procesos y solicitudes HTTP con la biblioteca estándar; el servidor, el cliente de carga y los binarios exportados que se miden son Rust. No forman parte del compilador ni de la toolchain distribuida.

## Trazabilidad de las capacidades

La matriz detallada de capacidades, nombres de pruebas y evidencia se mantiene como [artefacto del repositorio fuera del sitio](../tests/trazabilidad.md). Este resumen evita duplicar cientos de nombres de pruebas en la documentación pública. Una fila de la matriz acredita solo la evidencia que describe; no implica cobertura general.
