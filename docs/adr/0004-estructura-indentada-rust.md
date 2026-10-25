# ADR 0004: Items y coincidencia indentados preservan Rust

- **Estado:** Aceptada
- **Fecha:** 2026-10-25
- **Contexto:** Óxido busca una sintaxis estructural por indentación, pero no reemplaza el modelo de tipos, traits, propiedad ni compilación de Rust. Fase 7 solo admitía `struct`/`enum` indentados y exigía llaves Rust en métodos, traits y módulos. `match` no aceptaba brazos indentados.

## Decisión

Representar `impl`, `trait`, módulos y brazos de `coincidir` en el AST de Óxido. Reusar los nodos de función/bloque existentes para métodos y cuerpos, preservar expresiones, tipos, patrones y guards como fragmentos/tokens Rust, y generar árboles de tokens con `quote`/`proc_macro2`. Syn valida el contexto final; Cargo/rustc valida semántica.

Los métodos de traits admiten tanto firma terminada en `;` como cuerpo por defecto. Un brazo de `coincidir` admite una expresión en la misma línea o un bloque indentado después de `=>`. Los argumentos de macros estándar expresivas admiten normalización anidada; las DSL de terceros se mantienen opacas según la regla de [aliases](../aliases.md).

## Consecuencias

- La indentación es uniforme para métodos, módulos anidados y ramas de coincidencia.
- Se conserva la interoperabilidad y semántica de Rust; no se añade parser de tipos propio ni runtime.
- Las nuevas formas deben tener casos end-to-end positivos y negativos antes de ampliar la compatibilidad publicada.
- Features avanzadas (closures, macros propias, async, lifetimes) siguen sus subfases de Fase 8.
