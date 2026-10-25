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
- El comando `oxido formato` aún no está implementado; su aceptación exige idempotencia y conservación explícita de comentarios.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- LSP usa los mismos tokens y spans que el CLI.

## Interoperabilidad de ida y vuelta

- Un fixture del subconjunto reversible hace `.ox → .rs → .ox → .rs` y conserva comentarios ordinarios, documentación y semántica.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio documental de Fase 1

La gramática, modos, alias/reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas, y cada decisión tiene un criterio de aceptación asignado a una fase. Cerrar la fase documental no afirma que esos casos se hayan ejecutado. El harness inicial comienza en Fase 2 y se amplía en las fases siguientes.
