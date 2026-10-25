# Fase 3: fuentes y lexer

**Estado:** completada. La API produce tokens deterministas y diagnósticos con spans half-open de bytes UTF-8. El parser, AST y traducción `.ox → .rs` siguen en Fase 4/5.

## Entregado

- `SourceFile`, `Span` y conversión a línea/columna sin cortar caracteres UTF-8.
- Tokens para identificadores Unicode/raw, keywords inglés/español, keywords reservadas, números, cadenas/raw strings, caracteres, lifetimes, comentarios, puntuación, saltos, `INDENT`/`DEDENT` y encabezado de modo.
- Modos de proyecto y cabecera de archivo; error si faltan ambos o se contradicen.
- Indentación de cuatro espacios, rechazo de tabs y niveles/dedentación inválidos; en modo llaves la indentación visual no produce tokens estructurales.
- Comentarios anidados y grupos delimitados; marca como opacos los tokens dentro de invocaciones de macros y atributos, preservando el texto original mediante spans.
- Rechazo de controles bidi (`OX1002`) incluso dentro de comentarios y literales. Aviso `OX1003` para identificadores que mezclan Latin con griego o cirílico, política conservadora y explícita.
- Límites: 4 MiB por fuente, 256 grupos/comentarios anidados y 1.000.000 tokens. Errores de límites usan `OX6001`.
- Diagnósticos para tokens inválidos, literales/comentarios sin cierre, modo ausente/conflictivo y fallos de indentación.

## Pruebas

`crates/oxido-language/tests/lexer.rs` cubre spans UTF-8, alias, identificadores crudos, literales, comentarios anidados, operadores, ambos modos, headers, tabuladores, dedentación, Unicode confusable/bidi, entradas incompletas, límites y tokens opacos.

## Límites

- La clasificación de un alias como keyword es léxica; decidir si se usó en posición de identificador y sugerir `r#...` requiere contexto del parser.
- `OX1003` solo detecta mezcla de alfabeto latino con griego/cirílico; no implementa una tabla Unicode completa de confusables.
- La gramática/contexto de macros, atributos, bloques y expresiones se resuelve en Fase 4.
- El CLI todavía no expone un comando `lex` ni consume `.ox` en el flujo de compilación.
