# Fase 8 — Integración con Rust

**Estado:** en curso. Este informe registra la primera subfase implementada; no declara completada la Fase 8.

## Subfase 1 — Estructura indentada

**Estado:** completada y verificada localmente.

La sintaxis indentada inicial admite:

- `implementar Tipo:` con métodos, métodos asociados y items asociados Rust terminados en `;`.
- `rasgo Nombre:` con declaraciones de métodos terminadas en `;`, métodos por defecto y items asociados.
- `modulo nombre:` con items anidados recursivamente.
- `coincidir expresion:` como expresión independiente o inicializador `variable x = coincidir ...:`.
- Brazos `patron => expresion` y `patron =>` seguido de un bloque indentado.

Los encabezados, patrones, tipos y expresiones siguen tokens Rust con aliases Óxido documentados. Los macros conservan sus argumentos opacos. Syn valida cada item/contexto y Cargo/rustc mantiene la validación semántica.

Ejemplo combinado: [fase_8_estructura_indentada.ox](../examples/fase_8_estructura_indentada.ox). Se ejecuta con `cargo run --locked -p oxido-cli -- ejecutar examples/fase_8_estructura_indentada.ox`.

Validación local de esta entrega: el ejemplo se compiló y ejecutó con Cargo; `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings` pasan. El build del portal se valida por separado; los checks remotos se registrarán con el PR.

## Límites de esta entrega

- Los rasgos e implementaciones conservan las reglas de Rust para receivers, coherencia, tipos asociados y métodos requeridos.
- Los brazos conservan sintaxis Rust en patrones, guards y expresiones; solo su estructura externa usa indentación.
- Los argumentos de invocaciones de macros permanecen Rust opaco.
- `async`, macros declarativas, closures, lifetimes complejos, dependencias Cargo de proyectos reales e interoperabilidad general siguen en subfases posteriores.

La decisión arquitectónica está en [ADR 0004](adr/0004-estructura-indentada-rust.md). La aceptación global de Fase 8 requiere cerrar las subfases restantes en el [roadmap](roadmap.md), con pruebas y límites por característica.
