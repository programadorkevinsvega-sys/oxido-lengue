# Fase 3: fuentes y lexer inicial

**Estado:** completada. Al cierre de esta fase, la API producía tokens deterministas y diagnósticos con spans half-open de bytes UTF-8. El parser y AST se añadieron en Fase 4; la traducción `.ox → .rs` queda para Fase 5.

## Entregado

- `SourceFile`, `Span` y conversión a línea/columna sin cortar caracteres UTF-8.
- Tokens para identificadores Unicode/raw, keywords inglés/español, keywords reservadas, números, cadenas/raw strings, caracteres, lifetimes, comentarios, puntuación, saltos, `INDENT`/`DEDENT` y encabezado de modo.
- Modos de proyecto y cabecera de archivo; error si faltan ambos o se contradicen.
- Indentación de cuatro espacios, rechazo de tabs y niveles/dedentación inválidos; en modo llaves la indentación visual no produce tokens estructurales.
- Comentarios anidados y grupos delimitados; marca como opacos los tokens dentro de invocaciones de macros y atributos, preservando el texto original mediante spans.
- En modo indentación, elimina el salto estructural cuando una línea termina con operador/puntuación de continuación o la siguiente línea empieza con `.`/`::`; la indentación visual de esa línea no abre un bloque.
- Rechazo de controles bidi (`OX1002`) incluso dentro de comentarios y literales. Aviso `OX1003` para identificadores que mezclan Latin con griego o cirílico, política conservadora y explícita.
- Límites: 4 MiB por fuente, 256 grupos/comentarios anidados, 128 niveles de indentación y 1.000.000 tokens. El parser limita bloques recursivos a 128 niveles. Los límites producen `OX6001`, incluido el exceso de tokens; el límite exacto de un millón se acepta.
- Diagnósticos para tokens inválidos, literales/comentarios sin cierre, modo ausente/conflictivo y fallos de indentación.

## Pruebas

`crates/oxido-language/tests/lexer.rs` cubre spans UTF-8, alias, identificadores crudos, literales, comentarios anidados, operadores y continuaciones multilínea, ambos modos, headers, tabuladores, dedentación, Unicode confusable/bidi, entradas incompletas, límites de fuente/grupo/indentación/tokens, la frontera exacta de tokens y tokens opacos.

## Límites

- La clasificación de un alias como keyword es léxica; el parser de Fase 4 diagnostica nombres de función y bindings simples de `let`, y Fase 6 ampliará la comprobación a todos los contextos de identificador.
- `OX1003` solo detecta mezcla de alfabeto latino con griego/cirílico; no implementa una tabla Unicode completa de confusables.
- El parser de Fase 4 reconoce el subconjunto inicial descrito en su informe; estructuras y formas de lenguaje fuera de ese alcance siguen pendientes de sus fases correspondientes.
- El CLI todavía no expone un comando `lex` ni consume `.ox` en el flujo de compilación.
