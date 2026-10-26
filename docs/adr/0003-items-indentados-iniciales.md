# ADR-0003: AST y generación tokenizada para items indentados

- **Estado:** Aceptada
- **Fecha:** 2026-10-25
- **Fase:** 7, lenguaje inicial

## Contexto

La fase de lenguaje inicial necesita permitir `struct`, `enum` y `use` con bloques indentados sin reemplazar texto Rust de forma global ni duplicar el parser completo de Rust. Syn y `rustc` deben seguir siendo la fuente de verdad para la sintaxis final y la semántica.

## Decisión

- `struct` y `enum` indentados tienen nodos explícitos en el AST: kind, visibilidad, atributos, encabezado, miembros y spans.
- El parser reconoce los límites de cada campo/variante y conserva su fragmento con span. No concatena texto para fabricar un item Rust.
- En la entrega inicial, el backend generaba los items con `quote`/`proc_macro2`, normalizaba aliases fuera de grupos opacos, parseaba el item con Syn y aplicaba `prettyplease` al archivo.
- `use` indentado tiene un nodo AST propio; el backend emite el `;` Rust aunque el archivo Óxido omita ese terminador.
- Al aprobar este ADR, `impl`, `trait` y módulos se preservaban como items Rust con llaves; sus cuerpos indentados se implementaron después en el alcance documentado de Fase 8.

## Evolución de la implementación

La salida actual también pasa por `rustfmt` 1.99.1 después de `prettyplease`; ver la implementación y los criterios de Fase 9 en [roadmap.md](../roadmap.md) y [testing.md](../testing.md).

## Consecuencias

- La generación mantiene mapas de posiciones para cada encabezado, miembro e item, y diagnósticos de `rustc` pueden volver a campos `.ox`.
- El parser Óxido cubre una superficie pequeña; campos y variantes siguen usando la gramática Rust.
- La cobertura de items indentados crece de forma explícita; no se infiere soporte para toda la gramática de items de Rust.

## Evidencia

La implementación y los ejemplos ejecutables están descritos en [Fase 7](../fase-7-lenguaje-inicial.md). Las pruebas del parser verifican la forma AST y los tests del compilador ejecutan structs, enums, imports y diagnósticos mapeados.
