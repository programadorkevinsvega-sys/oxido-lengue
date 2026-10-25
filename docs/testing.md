# Criterios de aceptación de Fase 1

Estos son casos de prueba redactados para implementar en Fases 2–5. No son pruebas ejecutadas y no afirman que exista compilador.

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

- `matches!`, `println!`, macros Serde y macro desconocida conservan el nombre y todos los tokens interiores.
- Alias escritos dentro de argumento macro/atributo no cambian.
- Grupos `()[]{}` anidados se conservan.
- Macros braced no se confunden con bloques del modo.

## Parser, backend y diagnóstico

- Tipos, patrones y expresiones pasan a Syn y se exige consumo completo del fragmento.
- Archivo Rust generado parsea como `syn::File`; después Cargo/rustc decide semántica.
- Cada token emitido conserva span; tokens sintéticos mapean a introductor de bloque.
- Error Rust en archivo generado mapea a rango `.ox` mediante JSON, no mediante texto de terminal.
- Error de dependencia externa conserva ruta y código externos.
- LSP usa los mismos tokens y spans que el CLI.

## Interoperabilidad de ida y vuelta

- Un fixture del subconjunto reversible hace `.ox → .rs → .ox` y conserva AST/semántica.
- Fixture con macro, atributo o sintaxis no reversible falla sin pérdida silenciosa y explica el motivo.

## Criterio para cerrar Fase 1

La gramática, modos, alias/reservadas, política de macros, compatibilidad, errores, mapeo y pipeline tienen decisiones no ambiguas; esta lista tiene casos que cubren cada decisión. Cerrar la fase documental no requiere fingir que estas pruebas ya se ejecutaron. La implementación de fixtures/harness comienza en Fase 2.
