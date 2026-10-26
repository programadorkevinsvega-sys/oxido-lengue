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
| `lsp` | Bytes JSON-RPC, hasta el máximo de frame | No debe panicar al decodificar/despachar mensajes; limita el tamaño de frame a 4 MiB y acota a dos respuestas por mensaje |

Los seeds versionados están en `fuzz/corpus/{lexer,parser,config,backend,lsp}`. El target LSP llama el dispatcher JSON-RPC ya separado en una biblioteca; no ejercita todavía el framing `Content-Length` sobre stdio. Corpus descubierto, crashes, cobertura y binarios se mantienen fuera del repositorio hasta revisarse. El job `fuzz-smoke` ejecuta 1.000 casos con límite de entrada de 8 KiB por target usando cargo-fuzz 0.13.2 y nightly 2026-10-16. Es un smoke determinista acotado, no reemplaza fuzzing continuo ni demuestra ausencia de bugs. Siguen pendientes pruebas del framing LSP, cancelación y recuperación.

Las regresiones `differential_indented_ox_and_equivalent_rust_have_the_same_observable_result` y `differential_indented_impl_and_match_have_the_same_result_as_rust` compilan y ejecutan dos muestras en Óxido y Rust nativo, y comparan stdout. Cubren expresiones/iteradores/colecciones y estructuras/impl/métodos/match. Son evidencia de esos fixtures concretos, no certifican equivalencia semántica general.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- El dispatcher LSP tiene pruebas unitarias básicas de `didOpen`, diagnósticos y semantic tokens. El transporte stdio limita `Content-Length` a 4 MiB. Los semantic tokens siguen una heurística incompleta; faltan pruebas de framing, cambios incrementales, posiciones exhaustivas y cancelación.

## Interoperabilidad de ida y vuelta

- Un fixture del subconjunto reversible hace `.ox → .rs → .ox → .rs` y conserva comentarios ordinarios, documentación y semántica.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio documental de Fase 1

La gramática, modos, alias/reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas, y cada decisión tiene un criterio de aceptación asignado a una fase. Cerrar la fase documental no afirma que esos casos se hayan ejecutado. El harness inicial comienza en Fase 2 y se amplía en las fases siguientes.
## Orquestadores de regresión

Los scripts Python de `prueba/crud_sqlite_rust` y `prueba/crud_sqlite_export_rust` son lanzadores de pruebas de extremo a extremo. Se mantienen en Python porque solo coordinan procesos y solicitudes HTTP con la biblioteca estándar; el servidor, el cliente de carga y los binarios exportados que se miden son Rust. No forman parte del compilador ni de la toolchain distribuida.
