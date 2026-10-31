# Criterios de aceptación de Fase 1

Estos criterios pertenecen a las fases de implementación indicadas en [roadmap.md](roadmap.md). Son contratos de aceptación, no evidencia de que exista una función ni pruebas ejecutadas. Las pruebas que dependen de generación, Cargo, aliases ampliados o LSP se implementan en sus fases correspondientes.

| Área del contrato | Fases responsables |
|---|---|
| Configuración declarativa | 2; descubrimiento e integración con fuentes, 5 |
| Tokens, modo, Unicode e indentación | 3 |
| AST, bloques, controles y recuperación | 4 |
| Generación, semántica `rustc`, mapas y ida/vuelta | 5 |
| Normalización completa de aliases, colisiones y equivalencia de modos | 6 |
| Corpus de conformidad, fuzzing y CI extendido | 10 y 12 |
| Semantic tokens, diagnósticos y Workspace Trust en VS Code/LSP | 11 |

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
- `cargo_commands_emit_structured_json_for_success_and_source_diagnostics` comprueba el esquema JSON v1 de `compilar`/`ejecutar`/`probar` (la prueba ejecuta `ejecutar` y `compilar`), stdout exclusivamente JSON, salida saneada y ruta/línea/columna del `.ox` para un error de tipos.

`--json` se admite en `compilar`, `ejecutar` y `probar`. Un error temprano se representa con `OX0001`; un fallo de Cargo sin diagnóstico mapeable usa `OX5001`. Los comandos ajenos a Cargo no prometen salida estructurada de éxito. El formato indentado normaliza sangría, espaciado y grupos multilínea; preserva grupos opacos de macros, comentarios y literales. Para las construcciones de la matriz, el Rust exportado coincide byte por byte con `rustfmt` 1.99.1. La presentación `.ox` sigue las convenciones de layout documentadas y probadas de Óxido; no pretende ser texto Rust idéntico, pues su sintaxis y palabras clave difieren.

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

## Informe reproducible

Desde la raíz, ejecuta:

```sh
cargo run --manifest-path tools/validation-report/Cargo.toml --bin oxido-validation-report --locked
```

El ejecutor registra commit, rama, estado limpio/modificado, sistema, toolchains, comandos, códigos de salida, duración y totales de tests que Cargo reporta. Incluye sus propios fmt/Clippy/tests y los del workspace, cargo-deny, build/smoke de fuzz y build del sitio. El informe se escribe en `target/validation-report.md`; una suite que falle deja al comando con código distinto de cero. Si una herramienta opcional no está instalada, su estado es **No evaluada**. No copia logs de procesos, por lo que evita filtrar rutas absolutas; solo publica los nombres de las pruebas fallidas como identificadores reproducibles. Este reporte no mide rendimiento y no consulta CI remota desde local. El benchmark release comparativo es una ejecución aparte; los resultados de 2026-10-26 están en [`informe de benchmark`](reports/fase-10/2026-10-26-benchmark-release-github.md). No presentes el reporte de validación como sustituto de resultados remotos.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- El LSP anuncia sincronización incremental y aplica cambios por rangos UTF-16 en secuencia, ignora versiones obsoletas y rechaza rangos inválidos sin alterar el documento. Hay regresiones para recuperación completa/incremental, rangos partidos dentro de un par sustituto y versiones antiguas. El decoder stdio limita cabeceras a 8 KiB y `Content-Length` a 4 MiB; la cola de eventos es acotada. Semantic tokens se construyen desde los tokens del lexer compartido; el índice de declaraciones modela ámbitos locales acotados de parámetros y enlaces `dejar`/`let`/`para`/`for`, usa símbolos de archivos cerrados mediante una caché versionada fuera del workspace y, en workspace confiable, fusiona tokens de `rust-analyzer` con correspondencia token a token exacta. Lo no resuelto conserva la clase léxica. No se declara resolución de tipos universal. `$/cancelRequest` cancela cooperativamente el cálculo asíncrono de semantic tokens y devuelve `-32800`; los diagnósticos de `didOpen`/`didChange` usan `parse_ox` en un worker versionado y descartan resultados obsoletos. El índice local de navegación resuelve funciones, métodos, tipos y módulos con cualificadores, incluye referencias desde la declaración hacia llamadas cualificadas en otros archivos y devuelve todas las candidatas ante nombres repetidos ambiguos. `prepareRename`/`rename` delegan en `rust-analyzer` en workspace confiable y solo aplican ediciones con correspondencia exacta al `.ox`; el fallback local renombra un enlace de su ámbito o un símbolo global único y rechaza ambigüedad, nombres reservados e identificadores inválidos. La caché del índice se versiona, se invalida por fecha/tamaño/hash y se escribe de forma atómica fuera del workspace. `textDocument/formatting` indentado reutiliza el formateador de `oxido formato`. `oxido/check` exige confianza, usa el `oxido.toml` y el `Cargo.lock` del proyecto y agrupa cada diagnóstico de Cargo por el `.ox` de origen; el bucle LSP la despacha a un worker con una sola comprobación concurrente, de modo que otras solicitudes siguen respondiendo. `$/cancelRequest` marca también esa comprobación y mata el árbol de procesos de Cargo; el resultado cancelado no publica diagnósticos. La prueba host comprueba el `E0308` en `util_error.ox`, la navegación/formato en Restricted Mode, el rechazo de Cargo sin confianza, la respuesta/cancelación durante Cargo, tipos concretos en semantic tokens, completado, inlay hints, símbolos, referencias, edición incremental, documento incompleto y tres solicitudes concurrentes de tokens. Los benchmarks de latencia y la matriz Windows/macOS pertenecen a Fase 12.

## Interoperabilidad de ida y vuelta

- Un fixture del subconjunto reversible hace `.ox → .rs → .ox → .rs` y conserva comentarios ordinarios, documentación y semántica.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio documental de Fase 1

La gramática, modos, alias/reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas, y cada decisión tiene un criterio de aceptación asignado a una fase. Cerrar la fase documental no afirma que esos casos se hayan ejecutado. El harness inicial comienza en Fase 2 y se amplía en las fases siguientes.
## Orquestadores de regresión

Los scripts Python de `prueba/crud_sqlite_rust` y `prueba/crud_sqlite_export_rust` son lanzadores de pruebas de extremo a extremo. Se mantienen en Python porque solo coordinan procesos y solicitudes HTTP con la biblioteca estándar; el servidor, el cliente de carga y los binarios exportados que se miden son Rust. No forman parte del compilador ni de la toolchain distribuida.

## Matriz de trazabilidad de capacidades anunciadas

Esta matriz enlaza las áreas visibles en la documentación de compatibilidad con pruebas ejecutables por nombre. Los nombres corresponden a funciones de prueba; las pruebas no listadas aquí no se deben inferir como evidencia. `cargo test --workspace --locked` ejecuta las suites indicadas.

| Área | Pruebas identificables | Evidencia que aporta |
|---|---|---|
| Lexer, límites, Unicode y spans | `crates/oxido-language/tests/lexer.rs`: `tracks_utf8_spans_and_line_columns`, `reports_mode_conflicts_tabs_bidi_and_confusables`, `warns_on_fullwidth_identifiers_and_other_mixed_scripts`, `caps_diagnostics_and_reports_the_omitted_count`, `enforces_source_and_group_limits_with_diagnostics`, `enforces_the_token_limit_without_rejecting_the_exact_boundary`, `stops_lexing_when_the_deadline_expires_without_panicking`, `marks_macro_arguments_and_attribute_trees_opaque`; `crates/oxido-language/tests/parser.rs`: `stops_parsing_when_the_deadline_expires_without_panicking`, `caps_parser_diagnostics_and_reports_the_omitted_count`; `crates/oxido-compiler/src/lib.rs`: `caps_rust_diagnostics_and_reports_the_omitted_count`; `fuzz/fuzz_targets/lexer.rs` | Casos deterministas y smoke fuzz en ambos modos; bytes no UTF-8 se omiten porque la fuente es `str`. El plazo de análisis emite `OX6001` y no parsea tokens parciales. El tope de 1.000 diagnósticos conserva un resumen con los omitidos en lexer, parser y diagnósticos de Rust. Los confundibles cubren alfabetos mezclados, armenio y ancho completo. |
| Política de `unsafe` | `crates/oxido-cli/tests/cli.rs`: `audits_unsafe_tokens_and_can_reject_them_without_compiling`, `audits_the_oxido_keyword_alias_without_running_cargo`, `reports_project_risk_surface_without_leaking_absolute_paths` | `auditar-unsafe` y `auditar-proyecto` detectan el alias `inseguro`, rechazan con `--rechazar` y no filtran rutas absolutas. La auditoría es léxica y no ve expansiones macro. |
| Parser, modos y recuperación | `crates/oxido-language/tests/parser.rs`: `parses_indented_function_and_preserves_tail_semicolon_semantics`, `both_block_styles_keep_the_same_tail_and_semicolon_semantics`, `parser_reports_incomplete_headers_and_recovers_to_next_function`, `rejects_function_bodies_with_delimiters_from_the_other_mode` | Regla del `;`, modo con llaves/indentación, diagnóstico y recuperación al siguiente elemento. |
| Alias, reservadas y escape | `crates/oxido-language/tests/parser.rs`: `diagnoses_reserved_aliases_used_as_function_and_binding_names`, `future_reserved_aliases_are_rejected_as_names_with_raw_escape_suggestions`; `crates/oxido-compiler/tests/pipeline.rs`: `prelude_aliases_collide_as_identifiers_with_raw_escape_guidance`, `declared_spanish_method_names_are_not_rewritten_as_standard_aliases`, `texto_is_a_variable_name_and_a_type_alias_only_in_type_position`, `error_trait_name_is_not_rewritten_as_the_err_variant_alias` | Errores/sugerencias, colisiones contextuales y preservación de nombres declarados. |
| Backend, tipos, bloques e interoperabilidad | `crates/oxido-compiler/tests/pipeline.rs`: `parses_ox_structure_and_validates_rust_fragments_with_syn`, `compiles_and_runs_indented_structs_enums_and_collections`, `compiles_and_runs_indented_impl_traits_modules_and_match_arms`, `impl_methods_generics_associated_items_tail_values_and_block_modes_agree`, `compiles_all_published_standard_macro_aliases_with_cargo_and_rustc` | Syn/Cargo/rustc comprueban fragmentos y ejecuciones de los casos nombrados. No equivale a cubrir toda la gramática Rust. |
| Macros externas/procedurales | `crates/oxido-cli/tests/cli.rs`: `interoperates_with_registry_proc_macros_in_direct_and_exported_module_trees`; pipeline: `compiles_indented_declarative_macros_and_derive_attributes_as_rust` | Casos fijados documentados en [compatibility.md](compatibility.md), con compilación directa y exportada para el fixture de crates reales. |
| Diferencial y diagnósticos frente a Rust | `crates/oxido-compiler/tests/pipeline.rs`: las cuatro pruebas diferenciales descritas arriba; `maps_rustc_errors_inside_indented_methods_to_the_method_line`, `maps_a_rust_type_error_in_an_indented_struct_field_back_to_ox`; `crates/oxido-cli/tests/cli.rs`: `cargo_commands_emit_structured_json_for_success_and_source_diagnostics` | Siete pares ejecutables comparan stdout y un fixture negativo compara E0308/mapa de línea. Es una muestra identificable, no una prueba de equivalencia general. |
| Módulos, árboles mixtos e importación/exportación | `crates/oxido-cli/tests/cli.rs`: `compiles_runs_and_recursively_exports_a_hierarchical_ox_project`, `import_project_rewrites_path_and_keeps_existing_ox_sources_in_a_mixed_tree`, `resolves_path_modules_and_exports_their_rewritten_rust_paths`, `rejects_path_modules_that_escape_the_project_root`, `import_project_rejects_a_symlink_output_destination` | Compilación/ejecución de árboles, `#[path]` relativo, importación reversible del subconjunto y rechazo de rutas no permitidas. |
| Formato y conservación | `crates/oxido-cli/tests/cli.rs`: `formats_braced_source_idempotently_without_losing_comments`, `formats_indented_source_idempotently_without_moving_comments_or_literals`, `formats_indented_source_with_rust_like_spacing_and_canonical_nested_indentation`, `formatter_conformance_matrix_matches_rustfmt_after_rust_export`; `crates/oxido-compiler/src/application/format.rs`: `normalizes_spacing_and_nested_indentation`, `preserves_comments_and_multiline_literals`; LSP: `lsp_formatting_uses_the_same_indented_formatter_as_the_cli` | El formateador indentado vive en `oxido-compiler` y lo usan CLI y LSP; idempotencia, comentarios/literales, texto canónico exacto y comparación del Rust exportado con rustfmt en los fixtures de la matriz. |
| Configuración, rutas y fuentes de dependencias | `crates/oxido-compiler/tests/config.rs`: `rejects_configuration_larger_than_the_resource_limit`, `rejects_local_dependency_symlinks_that_escape_the_project_root`, `rejects_git_url_and_custom_registry_sources`; CLI: `cargo_detects_modified_vendor_and_lockfile_checksums_during_compile` | Límites de configuración, confinamiento de rutas, fuentes rechazadas y verificación de checksums por Cargo. |
| Transporte LSP y resiliencia de entradas | `crates/oxido-lsp/src/lib.rs`: `handles_lifecycle_and_rejects_malformed_json_without_panicking`, `initialize_reads_the_project_mode_as_data_from_the_workspace_root`, `maps_compiler_spans_to_utf16_and_keeps_the_original_ox_uri`, `recovers_diagnostics_after_a_full_document_change`, `applies_sequential_incremental_changes_using_utf16_positions`, `rejects_invalid_incremental_ranges_without_corrupting_the_document`, `ignores_stale_document_versions`, `cancel_request_notification_marks_the_matching_request_token`, `canceled_semantic_token_work_stops_and_returns_request_cancelled`, `semantic_tokens_resolve_project_symbols_instead_of_only_lexical_classes`, `semantic_tokens_use_symbols_from_closed_workspace_files`, `rename_local_binding_only_edits_its_lexical_scope`, `rename_local_binding_skips_shadowed_and_qualified_names`, `rename_rejects_ambiguous_cross_file_symbols`, `rename_rejects_a_reserved_new_name`, `resolves_qualified_repeated_functions_to_distinct_modules`, `resolves_nested_type_and_associated_method_through_qualified_path`, `repeated_references_do_not_cross_into_another_qualified_module`, `resolves_module_declaration_and_reverse_references_across_ox_files`, `ambiguous_unqualified_name_reports_every_cross_module_candidate`, `completion_and_document_symbols_include_types_methods_and_modules`, `lsp_formatting_uses_the_same_indented_formatter_as_the_cli`, `rust_analyzer_errors_do_not_replace_local_results`, `stdio_server_processes_requests_and_writes_framed_responses`, `rejects_duplicate_missing_oversized_and_truncated_frames`, `rejects_a_header_line_over_the_limit_without_reading_the_body`, `bounds_a_burst_of_changes_to_one_queued_diagnostic`; `crates/oxido-lsp/src/symbols.rs`: `tracks_parameters_local_bindings_and_inner_shadowing`, `tracks_braced_let_and_for_bindings`; `crates/oxido-lsp/src/index.rs`: `persists_and_reuses_symbols_within_the_cache_directory`, `invalidates_an_entry_when_content_size_or_mtime_changes`, `never_writes_cache_inside_the_workspace_root`, `ignores_a_cache_with_a_different_schema_version`, `restricts_cache_permissions_to_the_current_user`; `crates/oxido-rust-backend/src/lib.rs`: `maps_individual_fragment_tokens_for_precise_rename_ranges`; `fuzz/fuzz_targets/lsp.rs` | Parser/lexer compartidos, spans UTF-16, recuperación completa/incremental, diagnósticos asíncronos versionados, cancelación de semantic tokens, navegación cualificada/repetida, ámbitos locales, renombrado local/ambiguo/reservado, persistencia e invalidación de caché, mapeo token a token para rename, símbolos por tipo y formato compartido con la CLI. |
| Cancelación de Cargo y respuesta del bucle | `crates/oxido-cargo/src/lib.rs`: `enforces_the_process_timeout`, `honors_a_cancellation_flag_before_spawning_cargo`; `crates/oxido-lsp/src/lib.rs`: `cancel_request_notification_marks_a_pending_cargo_check`, `cargo_check_requires_explicit_workspace_trust`; host: caso `cancellation` en `test/host/extension.test.js` con `prepareSlowFakeCargo` y `OXIDO_CARGO_BINARY` | El adaptador no lanza Cargo si la bandera ya está activa y mata el grupo al cancelar durante la ejecución; `$/cancelRequest` marca la solicitud pendiente. La prueba host exige que `oxido/stats` responda durante Cargo, que la solicitud cancele con `-32800` y que el marcador de finalización no exista. |
| Cliente VS Code y Workspace Trust | `editors/vscode-oxido/test/manifest.test.js`: asociación `.ox`, comandos, configuración restringida, ausencia de rutas de Cargo/shell, empaquetado VSIX fijado y resolución del servidor incluido; `test/host/extension.test.js`: activación, diagnóstico real, solicitudes de hover, completado, inlay hints, definición, semantic tokens y `prepareRename`/`rename` en VS Code 1.143.1; hover tipado de la variable local `valor` (`i32`), definición de esa variable en `semantic.ox`, hover con firma Rust (`pub fn saludo()`) entre módulos, navegación `util::saludo()` a `util.ox` y contador que exige respuesta real de `rust-analyzer`; tipos concretos de módulo, estructura, función y referencia cualificada en `semantic_tokens.ox`, más `documentSymbol` y `references`; caso Cargo con `itoa` vendorizado, `Cargo.lock` intacto y `E0308` en `util_error.ox`; caso de navegación/formato en Restricted Mode que resuelve `alpha`/`beta`, `dominio::modelos::Alumno::nuevo`, la declaración del módulo `modelos`, referencias inversas desde `alpha.ox` y la ambigüedad sin cualificar de `ambigua.ox`, exige el formato canónico exacto, completado local, inlay hints, símbolos, edición incremental, documento incompleto `OX1001` y tres solicitudes concurrentes de tokens; caso de cancelación Cargo; casos VSIX estándar y VSIX con servidor incluido; caso Restricted Mode con `cargo` falso que no deja marcador; `test/run-host.js` se ejecuta en CI | La prueba host cubre un workspace confiable, Cargo real, navegación/formato local sin confianza, cancelación Cargo, un VSIX estándar instalado, un VSIX con `oxido-lsp` incluido y un workspace no confiable. En el workspace confiable exige tipos y firmas reales de `rust-analyzer` mapeados al `.ox`, contenido semántico en los tokens, un `WorkspaceEdit` de renombrado mapeado a `semantic_tokens.ox`, `indexFiles > 0` y `indexCacheWritten === true` o `indexCacheReused > 0`; `run-host.js` fija `OXIDO_CACHE_DIR` fuera del fixture y falla si la caché aparece dentro del workspace. El caso restringido exige `prepareRename`/`rename` locales sin iniciar `rust-analyzer`. `cargo_check_requires_explicit_workspace_trust` y `untrusted_workspace_never_starts_rust_analyzer` cubren la denegación en el servidor; `maps_cargo_diagnostics_to_their_sources_and_utf16_columns`, `cargo_mapping_preserves_utf16_columns_and_omits_failure_notes` y `successful_cargo_check_without_diagnostics_has_no_synthetic_error` cubren el mapeo. La compilación Cargo usa una dependencia vendorizada concreta y no demuestra descarga desde crates.io. La matriz Windows/macOS, firma, notarización, publicación y benchmarks multiplataforma pasan a Fase 12. |
| Empaquetado e instalación VSIX | `editors/vscode-oxido/test/run-host.js`: `packageVsix`, `packageBundledVsix`, `installVsix`, casos `installed` y `bundled`; `test/host/extension.test.js`: `runInstalledCase`; `test/manifest.test.js`: `packaging pins vsce and excludes test-only material from the VSIX` y `resolves a bundled server only for the default path and an executable file` | `npm run package:vsix` produce el VSIX estándar con vsce 3.9.2 y `.vscodeignore`; `npm run package:vsix:server` produce un VSIX por plataforma con `server/oxido-lsp` (o `.exe`) y checksum opcional. `npm run test:vsix` instala el estándar con la CLI de VS Code 1.143.1 en un perfil limpio y exige que la extensión activa proceda del directorio instalado, que no incluya `test/`, `.vscode-test/`, `scripts/`, `dist/` ni el lockfile, y que `main.ox` reciba `OX1002` desde `oxido-lsp`. `npm run test:vsix:server` repite la instalación con el servidor incluido y un `PATH` sin `oxido-lsp`; solo se ha ejecutado en Linux x64. El workflow `.github/workflows/release.yml` prepara la matriz y los checksums; su ejecución fuera de Linux, la firma, la notarización y la publicación son criterios de Fase 12. |

Si una prueba se renombra o elimina, actualiza esta tabla en el mismo cambio. Una fila acredita únicamente lo que expresa su columna de evidencia y no debe ampliarse por analogía.
