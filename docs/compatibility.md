# Compatibilidad de Óxido 0.1 con Rust

Esta matriz describe el **alcance de la implementación inicial**, no funcionalidades que existan hoy. Rust 2024 es la edición objetivo. La aceptación exige pruebas positivas/negativas y compilación con Cargo/rustc cuando aplique.

| Área | Decisión 0.1 | Estado actual del repositorio |
|---|---|---|
| Funciones libres | Estructura bilingüe en ambos modos; firma preservada como Rust | Parcial: parser y validación Syn; sin generación |
| Variables `let` | Alias `dejar`; patrón/tipo/expresión son Rust; terminador inferido solo en indentation | Parcial: AST/validación Syn en cuerpos parseados |
| Bloques | `:`+indent o `{}` según modo, nunca mezclados como bloques | Parcial: ambos estilos producen AST |
| Valor del bloque | Última expresión sin `;` retorna valor; con `;` devuelve `()` | Parcial: AST distingue `tail` y terminador; falta código generado |
| `if/else`, `loop`, `while`, `for` | Keywords bilingües y bloques en ambos modos | Parcial: estructura y cabeceras validadas con Syn |
| Tipos y referencias | Sintaxis Rust preservada y parseada; semántica a `rustc` | Parcial: fragmentos de tipo validados con Syn en `let` |
| `struct`, `enum`, `impl`, `trait`, `mod`, `use` | Rust passthrough en braces; soporte indentado solo para bloques estructurales listados en gramática | Parcial: passthrough opaco en modo llaves; no en indentación |
| Expresiones, patrones y genéricos | Tokens Rust; parser Syn; `rustc` hace validación semántica | Parcial: expresiones/patrones/firma validados en el subconjunto parseado |
| `match` | Keywords traducibles; brazos se escriben con `{}` también en modo indentation | No implementado |
| Macros/atributos | Árboles opacos; no se traducen nombre, argumentos ni contenido | Parcial: conservación en modo llaves; no expansión ni traducción |
| Macros procedurales | Cargo/rustc pueden ejecutarlas; no las analiza ni expande Óxido | No implementado |
| `async`/`await` | Alias reservados; solo se aceptan donde el Rust resultante compila | No implementado |
| `unsafe` | Semántica Rust intacta; el compilador Óxido prohíbe `unsafe` en su propio código y ofrecerá política para rechazarlo en fuente analizada | No implementado |
| Items Rust no listados | Braces: passthrough si Syn y rustc aceptan; indentation: rechazo salvo transformador estructural definido | No implementado |
| Tipos/métodos españoles | Sin alias ni prelude en 0.1 | Fuera de alcance |
| Alias de macros | Sin alias en 0.1 | Fuera de alcance |
| Sistema de tipos/runtime propios | No existen | Fuera de alcance |

“Parcial” significa que la estructura inicial se parsea o valida, pero aún no implica generación de Rust ni aceptación integral del programa. Un error de Syn significa incompatibilidad sintáctica detectada antes de Cargo; el éxito de Syn no garantiza que `rustc` acepte el programa ni que un crate/macros compile. Ver [informe de Fase 4](fase-4-parser.md).
