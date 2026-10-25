# Compatibilidad de Óxido 0.1 con Rust

Esta matriz describe el **alcance verificado**, no compatibilidad general con Rust. Rust 2024 es la edición objetivo. La aceptación exige pruebas positivas/negativas y compilación con Cargo/rustc cuando aplique.

| Área | Decisión 0.1 | Estado actual del repositorio |
|---|---|---|
| Funciones libres | Estructura bilingüe en ambos modos; firma preservada como Rust | Parcial: generación y roundtrip de funciones; Cargo/rustc verificado con Hola, mundo |
| Variables `let` | Alias `dejar`; patrón/tipo/expresión son Rust; terminador inferido solo en indentation | Parcial: generación, Syn y diagnóstico de tipo real mapeado a `.ox` |
| Bloques | `:`+indent o `{}` según modo, nunca mezclados como bloques | Parcial: ambos estilos generan AST; generación inicial por AST |
| Valor del bloque | Última expresión sin `;` retorna valor; con `;` devuelve `()` | Parcial: se preservan tail y control final; pruebas de AST y generación |
| `if/else`, `loop`, `while`, `for` | Keywords bilingües y bloques en ambos modos | Parcial: generación sintáctica; aliases de cabecera siguen pendientes de Fase 6 |
| Tipos y referencias | Sintaxis Rust preservada y parseada; semántica a `rustc` | Parcial: fragmentos validados con Syn; Cargo detecta y mapea un error de tipo |
| `struct`, `enum`, `impl`, `trait`, `mod`, `use` | Rust passthrough en braces; soporte indentado solo para bloques estructurales listados en gramática | Parcial: escritura Rust con atributos/visibilidad y macros se conservan y validan en modo llaves; aliases y cuerpos indentados siguen pendientes |
| Expresiones, patrones y genéricos | Tokens Rust; parser Syn; `rustc` hace validación semántica | Parcial: expresiones/patrones/firma validados en el subconjunto parseado |
| `match` | Keywords traducibles; brazos se escriben con `{}` también en modo indentation | No implementado |
| Macros/atributos | Árboles opacos; no se traducen nombre, argumentos ni contenido | Parcial: se conserva y genera la invocación; el ejemplo probado usa `println!`; no se expanden macros externas |
| Identificadores reservados y escape | `r#nombre` fuerza identificador; colisiones de alias reciben `OX3001` cuando corresponda | Parcial: `fn` y bindings simples de `let` sugieren `r#...`; patrones anidados y todos los contextos se amplían en Fase 6 |
| Macros procedurales | Cargo/rustc pueden ejecutarlas; no las analiza ni expande Óxido | No implementado |
| `async`/`await` | Alias reservados; solo se aceptan donde el Rust resultante compila | No implementado |
| `unsafe` | Semántica Rust intacta; el compilador Óxido prohíbe `unsafe` en su propio código y ofrecerá política para rechazarlo en fuente analizada | No implementado |
| Items Rust no listados | Braces: passthrough si Syn y rustc aceptan; indentation: rechazo salvo transformador estructural definido | No implementado |
| Tipos/métodos españoles | Sin alias ni prelude en 0.1 | Fuera de alcance |
| Alias de macros | Sin alias en 0.1 | Fuera de alcance |
| Sistema de tipos/runtime propios | No existen | Fuera de alcance |

“Parcial” significa que existe evidencia para un subconjunto, no que cualquier programa de esa categoría funcione. La rebanada end-to-end actual se documenta en [Fase 5](fase-5-compilador.md). Syn valida sintaxis; solo Cargo/rustc comprueba tipos y compilación. Importar soporta funciones libres reversibles y no conserva comentarios ordinarios ni formato original.
