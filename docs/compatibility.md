# Compatibilidad de Óxido 0.1 con Rust

Esta matriz describe el **alcance verificado**, no compatibilidad general con Rust. Rust 2024 es la edición objetivo. La aceptación exige pruebas positivas/negativas y compilación con Cargo/rustc cuando aplique.

| Área | Decisión 0.1 | Estado actual del repositorio |
|---|---|---|
| Funciones libres | Estructura bilingüe en ambos modos; firma preservada como Rust | Parcial: generación y roundtrip de funciones; Cargo/rustc verificado con Hola, mundo |
| Variables `let` | Alias `dejar`/`variable`; `mutable` equivale a `mut`, incluida pareja `variable mutable` | Implementado y probado en sintaxis/generación; ejecución incluida en ejemplo de prelude |
| Bloques | `:`+indent o `{}` según modo, nunca mezclados como bloques | Parcial: ambos estilos generan AST; generación inicial por AST |
| Valor del bloque | Última expresión sin `;` retorna valor; con `;` devuelve `()` | Parcial: se preservan tail y control final; pruebas de AST y generación |
| `if/else`, `loop`, `while`, `for` | Keywords bilingües y bloques en ambos modos | Parcial: aliases y ejecución equivalente verificados para funciones/controles; nuevas formas siguen pendientes |
| Tipos, referencias y borrowing | Sintaxis Rust preservada y parseada; semántica a `rustc` | Implementado para tipos y referencias Rust del corpus de funciones; préstamo probado end-to-end |
| Tuplas, arrays y tipos primitivos | Sintaxis Rust preservada; semántica a `rustc` | Implementado y probado en código ejecutado |
| `struct`, `enum`, `impl`, `trait`, `mod`, `use` | Rust passthrough; `struct`/`enum` pueden tener campos/variantes indentados | Parcial: `struct`/`enum` y `use` indentados; `impl` con métodos y módulos se conservan como items Rust con llaves; cuerpos indentados de impl/trait/módulo pendientes |
| Expresiones, patrones y genéricos | Tokens Rust; parser Syn; `rustc` hace validación semántica | Parcial: expresiones/patrones/tipos se validan con Syn; genéricos Rust se preservan donde el parser acepta la firma/item |
| `match` | Keyword bilingüe; los brazos conservan sintaxis de llaves Rust en ambos modos | Parcial: expresión `coincidir` ejecutada en modo indentación; no hay sintaxis indentada para brazos |
| Macros/atributos | Nombres allowlisted y atributos allowlisted; argumentos/contenido opacos | Alias de macros y `#[prueba]`, `#[permitir]`, `#[derivar]` implementados; `matches!` y DSLs externos opacos; no se expanden macros |
| Identificadores reservados y escape | `r#nombre` fuerza identificador; colisiones de alias reciben `OX3001` cuando corresponda | Parcial: sugerencia comprobada en funciones, parámetros, patrones anidados, bindings y `para`; contextos Rust adicionales siguen pendientes |
| Macros procedurales | Cargo/rustc pueden ejecutarlas; no las analiza ni expande Óxido | No implementado |
| `Result`, `Option`, `?` | Tipos estándar Rust, con aliases `Resultado`/`Opcion` y variantes incluidas en prelude | Alias de prelude inicial y operador `?` probados mediante generación; las rutas externas siguen siendo Rust |
| Colecciones estándar | `Vector`/`Cadena`, métodos comunes y módulos `std` con aliases documentados | Alias comprobados en generación; ejemplo e2e compila y ejecuta `Vector<Cadena>`, `agregar`, `largo` y macros |
| `async`/`await` | Alias reservados; solo se aceptan donde el Rust resultante compila | Parsing/generación posible según contexto Rust; sin runtime async propio ni matriz de ejecución |
| `unsafe` | Semántica Rust intacta; el compilador Óxido prohíbe `unsafe` en su propio código y ofrecerá política para rechazarlo en fuente analizada | No implementado |
| Items Rust no listados | Braces: passthrough si Syn y rustc aceptan; indentation: rechazo salvo transformador estructural definido | No implementado |
| Tipos/métodos españoles | Prelude inicial enumerado en [aliases](aliases.md); el resto sigue Rust | Parcial por diseño; no implica traducción de crates ni APIs de terceros |
| Mensajes rustc en español | Traducción por código de diagnóstico JSON | Conjunto inicial de códigos; mensajes desconocidos conservan texto original y todos preservan código/span |
| Sistema de tipos/runtime propios | No existen | Fuera de alcance |

“Parcial” significa que existe evidencia para un subconjunto, no que cualquier programa de esa categoría funcione. La rebanada end-to-end actual se documenta en [Fase 5](fase-5-compilador.md). Syn valida sintaxis; solo Cargo/rustc comprueba tipos y compilación. Importar soporta funciones libres reversibles y no conserva comentarios ordinarios ni formato original.
