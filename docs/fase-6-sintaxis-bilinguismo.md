# Fase 6 — Sintaxis de bloques y bilingüismo

**Estado:** completada para la estructura inicial de funciones y controles. La fase no declara que todos los items o expresiones de Rust tengan sintaxis Óxido equivalente.

## Alcance verificado

- Los modos `indentacion` y `llaves` producen la misma AST estructural para funciones y controles iniciales (`si`, `bucle`, `mientras`, `para`).
- Cabeceras de proyecto sin modo por archivo y conflictos entre el modo configurado y la cabecera se validan antes de continuar.
- Los aliases de keywords se normalizan como tokens en firmas, visibilidad, tipos, patrones, expresiones y items Rust passthrough.
- Los grupos de macros externas/desconocidas y atributos desconocidos permanecen opacos. Los nombres de macros estándar allowlisted y las expresiones que su contrato permite recorrer se traducen selectivamente desde Fase 7; `r#identificador` se conserva sin traducir.
- Colisiones de alias reciben `OX3001` y sugerencia `r#...` en nombres de función, parámetros, patrones anidados, bindings y patrones `para` cubiertos por el parser.
- Ambos modos compilan y ejecutan un caso equivalente con la misma salida. Se mantienen pruebas de mezcla de delimitadores y semicolon/tail.
- El Rust generado pasa por Syn y se formatea con `prettyplease`. No se reescribe el archivo `.ox` ni se altera su modo al generarlo.

## Límites que pasan a Fase 7

Al cerrar Fase 6, los items con cuerpo indentado y las nuevas formas de lenguaje quedaron para Fase 7. Fase 7 implementa cuerpos indentados iniciales para `struct` y `enum`; `impl`, `trait` y módulos con cuerpo indentado, además de sintaxis indentada para brazos de `match`, siguen fuera de ese subconjunto. En modo llaves, los items Rust passthrough admitidos se conservan y se normalizan fuera de grupos opacos.

En el alcance original de Fase 6, los grupos de macros y atributos eran opacos. La política vigente se amplió en Fase 7: macros estándar allowlisted admiten aliases selectivos; macros externas/desconocidas y DSLs no se reescriben. Los aliases de tipos y métodos de prelude incorporados después están enumerados en [aliases](aliases.md); no se traducen APIs de crates de terceros.

## Evidencia

La suite de `oxido-language` prueba reglas de bloques, cabeceras y sugerencias de identificadores; `oxido-rust-backend` prueba normalización y opacidad; `oxido-compiler` compila los dos modos y comprueba salida equivalente, firmas, visibilidad y items passthrough. La matriz de límites está en [compatibilidad](compatibility.md). La gramática prevista, incluida la sintaxis todavía pendiente, está en [grammar](grammar.md).
