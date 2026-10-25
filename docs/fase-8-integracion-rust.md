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

Los encabezados, patrones, tipos y expresiones siguen tokens Rust con aliases Óxido documentados. Los argumentos de macros estándar que reciben expresiones pueden normalizar aliases anidados; las DSL externas y macros no allowlisted conservan sus tokens opacos. Syn valida cada item/contexto y Cargo/rustc mantiene la validación semántica.

Ejemplo combinado: [fase_8_estructura_indentada.ox](../examples/fase_8_estructura_indentada.ox). Se ejecuta con `cargo run --locked -p oxido-cli -- ejecutar examples/fase_8_estructura_indentada.ox`.

Validación local de esta entrega: el ejemplo se compiló y ejecutó con Cargo; `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings` pasan. El build del portal se valida por separado; los checks remotos se registrarán con el PR.

La cobertura de regresión de los bloques `implementar` verifica receivers `mismo`, `&mismo` y `&mutable mismo`; funciones asociadas sin receiver; genéricos y `donde` junto al `:` que abre el bloque; varios bloques para el mismo tipo; visibilidad `publico`; atributos y comentarios `///`; constantes asociadas y tipos asociados en traits/implementaciones. También comprueba el rechazo de bloques `implementar` vacíos, la mala indentación, la regla del `;` final, el mismo resultado al compilar formas indentadas y con llaves, y el mapeo de errores de rustc a la línea original `.ox`.

La regresión de documentación compara el Rust generado para comentarios `///` antes del primer, intermedio y último método; `///` con líneas en blanco y `#[inline]`; bloques `/** ... */`; comentarios internos `//!`/`/*! ... */`; comentarios ordinarios al cerrar el bloque y comentarios al final de su encabezado. La prueba CLI de Fase 5 confirma que la documentación se conserva semánticamente como `doc` y que comentarios ordinarios se mantienen en la ida y vuelta fuera de grupos opacos.

## Subfases 3 y 4 — Genéricos, lifetimes, patrones, closures e iteradores

**Estado:** verificadas para el corpus descrito; no implican compatibilidad general con Rust.

`compiles_generics_lifetimes_closures_iterators_and_patterns_in_both_modes` ejecuta el mismo programa en modo indentación y llaves. Usa una función genérica con lifetime explícito y bound `where`, un parámetro `impl Fn`, `Iterator::find`, una closure, patrones `Some`/`None` y `coincidir`. Ambos programas producen `Ana`. La prueba confirma generación y compilación con Cargo/rustc, además de igualdad observable entre los dos modos.

Los tipos, expresiones, closures y patrones dentro de estas firmas y cuerpos siguen siendo sintaxis Rust validada por Syn y semántica delegada a rustc. No se afirma cobertura completa de lifetimes, traits, inferencia, patrones ni iteradores.

## Subfase 2 — Dependencias Cargo

**Estado:** en curso; corte inicial para dependencias locales por ruta.

`oxido.toml` admite `[dependencies.nombre] path = "ruta/relativa"`. Óxido resuelve la ruta canónica y rechaza escapes de la raíz del proyecto, incluso mediante enlaces simbólicos, rutas no directorio y `Cargo.toml` simbólicos/no regulares. El backend serializa el manifiesto generado como TOML estructurado y ejecuta `cargo check`/`run` en modo `--offline` y `--locked`. La prueba CLI compila y ejecuta un crate de fixture local sin red.

Este corte no admite aún versiones de crates.io/git, ni consume/versiona un `Cargo.lock` del proyecto. El lockfile se genera dentro del workspace temporal y desaparece al finalizar. Por ello no se declara configuración reproducible completa. Una dependencia local puede ejecutar `build.rs` o macros procedurales durante `compilar`/`ejecutar`; esos comandos son explícitos y no se invocan desde el LSP. `--offline` limita la resolución/descarga de Cargo, pero no aísla el código de build de la red ni del sistema; la sandbox y la política de confianza siguen pendientes.

## Límites de esta entrega

- Los rasgos e implementaciones conservan las reglas de Rust para receivers, coherencia, tipos asociados y métodos requeridos.
- Los brazos conservan sintaxis Rust en patrones, guards y expresiones; solo su estructura externa usa indentación.
- Las macros externas y las DSL no allowlisted permanecen opacas; los aliases de macros estándar y sus expresiones anidadas se describen en [aliases](aliases.md).
- `async`, macros declarativas, lifetimes complejos, dependencias Cargo de registro con lockfile persistente, crates de referencia e interoperabilidad general siguen pendientes.

La decisión arquitectónica está en [ADR 0004](adr/0004-estructura-indentada-rust.md). La aceptación global de Fase 8 requiere cerrar las subfases restantes en el [roadmap](roadmap.md), con pruebas y límites por característica.
