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
- `formatter_conformance_matrix_matches_rustfmt_after_rust_export` compara cuatro fixtures agrupados por construcciones:

| Fixture | Cobertura concreta |
|---|---|
| `items-y-tipos` | `use` anidado, `derive`, visibilidad, struct genérico, campos, enum, función genérica y `where` |
| `contenedores-y-control` | struct genérico, firma y tipo asociado de trait, `impl` genérico, constante asociada, métodos asociados (incluido receiver mutable), módulo, closure, iteradores, `match` por brazos indentados y expresión tail |
| `macros-comentarios-y-async` | `macro_rules!`, atributo, comentario, raw string multilínea, invocaciones de macros opacas, `async`/`.await`, binding mutable, array, `for`, `while` y `break` |
| `expresiones-y-patrones` | alias de tipo, constante, tupla, referencias, local mutable con tipo, cadenas de métodos, `enumerate`/`fold`, closure con parámetros destructurados, bloque Rust dentro de expresión, `if let`, `return`, `match` y patrones wildcard/binding |
| `control-y-bloques` | `loop`, `if`/`else`, `break` con valor, `continue`, tail expression y ramas con valor |

Cada fixture comprueba el texto `.ox` esperado, vuelve a pasar `oxido formato --comprobar` y exporta antes/después. Al normalizar ambos `.rs` con `rustfmt` del toolchain 1.99.1 (`--edition 2024`), exige igualdad carácter por carácter del Rust formateado. Después `rustc` compila ambos exports como biblioteca; el fixture de macros también compila como ejecutable y comprueba la salida exacta `6\n`. Esto verifica la preservación de tokens/semántica observable y la salida de `rustfmt` para estos fixtures. Como `.ox` tiene palabras clave y delimitación distintas, su texto no puede ser carácter por carácter igual al `.rs`; tampoco se afirma cobertura exhaustiva de toda expresión Rust que Syn admita. La prueba necesita `rustfmt` y `rustc` 1.99.1, instalados en CI.
- `import_project_rewrites_path_and_keeps_existing_ox_sources_in_a_mixed_tree` importa módulos `.rs`/`.ox` mezclados con `#[path]` relativo, ejecuta el proyecto importado y compila/ejecuta su exportación Rust.
- `import_project_rejects_a_path_attribute_that_escapes_without_creating_output` y `import_project_rejects_a_symlink_output_destination` prueban los rechazos de ruta antes de escribir.
- `cargo_commands_emit_structured_json_for_success_and_source_diagnostics` comprueba el esquema JSON v1 de `compilar`/`ejecutar`/`probar` (la prueba ejecuta `ejecutar` y `compilar`), stdout exclusivamente JSON, salida saneada y ruta/línea/columna del `.ox` para un error de tipos.

`--json` se admite en `compilar`, `ejecutar` y `probar`. Un error temprano se representa con `OX0001`; un fallo de Cargo sin diagnóstico mapeable usa `OX5001`. Los comandos ajenos a Cargo no prometen salida estructurada de éxito. El formato indentado normaliza sangría, espaciado y grupos multilínea en el corpus probado; preserva grupos opacos de macros, comentarios y literales. La matriz `formatter_conformance_matrix_matches_rustfmt_after_rust_export` especifica sus fixtures y compara el Rust normalizado antes/después con `rustfmt` 1.99.1. Es evidencia de conformidad para esos casos, no de equivalencia completa con `rustfmt`; ampliar la cobertura por todas las construcciones admitidas sigue siendo criterio pendiente de Fase 9.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- El servidor LSP inicial acepta mensajes JSON-RPC sobre stdio y publica diagnósticos léxicos al abrir/cambiar documentos. Sus semantic tokens usan una heurística incompleta; no son fiables para resaltado. Faltan pruebas de posiciones, semantic tokens y cambios incrementales.

## Interoperabilidad de ida y vuelta

- Un fixture del subconjunto reversible hace `.ox → .rs → .ox → .rs` y conserva comentarios ordinarios, documentación y semántica.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio documental de Fase 1

La gramática, modos, alias/reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas, y cada decisión tiene un criterio de aceptación asignado a una fase. Cerrar la fase documental no afirma que esos casos se hayan ejecutado. El harness inicial comienza en Fase 2 y se amplía en las fases siguientes.
## Orquestadores de regresión

Los scripts Python de `prueba/crud_sqlite_rust` y `prueba/crud_sqlite_export_rust` son lanzadores de pruebas de extremo a extremo. Se mantienen en Python porque solo coordinan procesos y solicitudes HTTP con la biblioteca estándar; el servidor, el cliente de carga y los binarios exportados que se miden son Rust. No forman parte del compilador ni de la toolchain distribuida.
