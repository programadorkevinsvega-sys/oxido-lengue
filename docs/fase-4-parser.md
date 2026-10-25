# Fase 4: parser y AST

**Estado:** completada para el subconjunto estructural inicial. Esta fase analiza estructura y valida fragmentos con Syn; todavía no genera Rust ni invoca Cargo.

## Implementación

- `oxido-language` contiene el AST y el parser que reutiliza los tokens y spans del lexer.
- Reconoce funciones, atributos/visibilidad básicos, declaraciones `let`, `return`/`break`/`continue`, expresiones de línea, bloques y controles `if`/`else if`/`else`, `loop`, `while` y `for`.
- Conserva firmas, expresiones, patrones y tipos como fragmentos Rust con spans de origen. El backend valida esos fragmentos con Syn; esto comprueba sintaxis, no tipos, macros expandidas ni compatibilidad semántica con `rustc`.
- En modo llaves conserva items Rust y llamadas a macros como fragmentos opacos. Los argumentos de macros no se traducen.
- El AST separa statements de la expresión final (`tail`): en bloques indentados, una expresión final sin `;` es el valor del bloque; con `;` se queda como statement y el bloque no tiene tail. Los delimitadores se conservan en el AST.
- Los errores de parseo producen diagnósticos con spans y el parser intenta recuperarse para continuar.

## Evidencia

Las pruebas de `crates/oxido-language/tests/parser.rs` cubren funciones en ambos modos, semicolon/tail, control anidado, atributos, visibilidad, items passthrough, macros, Unicode y recuperación. `crates/oxido-compiler/tests/pipeline.rs` cubre integración con Syn y rechazo de una expresión mal formada.

## Límites conocidos

- El parser no es una gramática completa de Rust. En modo indentación, los items superiores distintos de funciones no están implementados.
- `match`, continuaciones generales de expresiones, declaraciones `use` y otros items estructurales no tienen transformación bilingüe completa.
- Los alias aparecen como keywords en el lexer, pero la normalización de fragmentos Rust y la generación mediante tokens quedan para Fase 5 y fases de expansión.
- Syn no ejecuta `rustc`, `build.rs` ni macros; tampoco sustituye la validación semántica.

La matriz de alcance por característica está en [compatibilidad](compatibility.md). La siguiente fase añade generación `.ox → .rs`, mapas de posiciones e integración Cargo.

La decisión arquitectónica de esta frontera está registrada en [ADR-0001](adr/0001-parser-hibrido.md).
