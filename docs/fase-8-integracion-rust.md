# Fase 8 — Integración con Rust

**Estado:** completada para el alcance inicial verificado. Esto no declara compatibilidad general con Rust ni preparación para producción.

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

**Estado:** verificada para el alcance de fuentes `path` y registry de crates.io.

`oxido.toml` admite una dependencia `[dependencies.nombre]` con exactamente `path` relativo al proyecto o `version` SemVer de crates.io; también permite `features` y `default-features`. Se rechazan URLs/git, opciones desconocidas, requisitos SemVer inválidos y mezclar ambas fuentes. Las rutas locales se canonicalizan y se rechazan escapes, incluidos enlaces simbólicos.

`oxido descargar-dependencias archivo.ox` resuelve y persiste `Cargo.lock`; `--actualizar` es explícito para actualizarlo. El archivo se escribe con staging y no sigue symlinks. `compilar` y `ejecutar` usan `--locked --offline` y no crean el lockfile implícitamente cuando hay registry dependencies. La prueba E2E usa crate vendorizado `itoa` con `CARGO_HOME` temporal vacío, checksum del lockfile, ejecución sin red, lockfile inmutable y exportación `.rs` idéntica en dos ejecuciones. Se verificó `semver` (`MIT OR Apache-2.0`) y su checksum, y cargo-deny/cargo-audit revisan advisories.

Cargo puede ejecutar `build.rs` o macros procedurales en las operaciones explícitas de resolver/compilar/ejecutar. `--offline` evita descargas, no aísla el proceso. Workspace Trust y sandbox no se implementan en esta fase de CLI; el LSP no debe iniciar compilaciones en workspaces no confiables.

## Subfase 5 — Macros declarativas y atributos

**Estado:** verificada para preservación opaca y delegación a Rust.

Las definiciones `macro_rules!` se preservan como fragmentos Rust opacos y Cargo expande la macro del fixture durante una ejecución E2E. Una prueba aparte consume una macro exportada por un crate local mediante una ruta calificada; el crate de la biblioteca no necesita cambios y los argumentos repetidos llegan intactos a su expansión. Los atributos aceptados por el subconjunto estructural también se conservan; `#[derive(Debug)]` está cubierto sobre una estructura indentada. Óxido no traduce ni reescribe los tokens de definiciones ni llamadas externas. Las pruebas verifican generación Rust y salida exacta del binario.

Esto no implementa traducción de sintaxis Óxido dentro de las reglas de macro, expansión propia, macros procedurales ni compatibilidad general con atributos de terceros. La lógica de la macro debe escribirse con sintaxis Rust; los atributos siguen sujetos a aceptación de Syn y rustc. La macro externa se prueba con un crate local por ruta; el corpus de registry cubre `itoa` y `futures-executor`, no macros exportadas desde crates de registry.

## Subfase 6 — Async/await y concurrencia

**Estado:** verificada para futures inmediatamente listos y concurrencia estándar; sin runtime propio.

La prueba E2E con la dependencia real `futures-executor` vendorizada ejecuta un future que usa `.await` y compara la salida. También valida el Rust generado (`async fn`, `.await`) con rustc; la tarea `std::thread::spawn` ejecuta y comprueba su resultado. Óxido no incorpora runtime async, executor, scheduler ni aliases de concurrencia: el proyecto elige la biblioteca/runtime y la semántica de suspensión sigue delegada a esa dependencia.

## Subfase 8 — Política y auditoría de `unsafe`

**Estado:** verificada para el subconjunto detectable documentado.

`oxido auditar-unsafe <archivo.ox|archivo.rs> [--rechazar]` muestra posición de cada token y puede rechazarlo sin compilar. `oxido auditar-proyecto <directorio>` enumera tokens `unsafe`/`inseguro`, archivos `build.rs`, manifiestos que declaran crates `proc-macro`, bloques FFI y llamadas léxicas `Command::new`. Omite directorios de build comunes, no sigue symlinks, limita el número de entradas y muestra rutas relativas para no filtrar rutas absolutas.

El informe es estático y conservador: no resuelve todas las dependencias registry ni relaciones indirectas con macros procedurales, puede contar código de macros opacas, no ve código generado por expansión y solo reconoce la forma visible `Command::new`. No prueba ausencia de riesgo ni reemplaza sandbox, Workspace Trust o revisión de dependencias.

## Subfase 9 — Matriz por edición y versiones

**Estado:** completada para la edición declarada y el corpus probado.

La matriz de [compatibilidad](compatibility.md) declara Rust 2024, MSRV 1.85 y las dependencias E2E verificadas. No implica compatibilidad con todas las versiones de Rust o crates; `Cargo.lock` y las pruebas por característica son la evidencia reproducible.

## Límites de esta entrega

- Los rasgos e implementaciones conservan las reglas de Rust para receivers, coherencia, tipos asociados y métodos requeridos.
- Los brazos conservan sintaxis Rust en patrones, guards y expresiones; solo su estructura externa usa indentación.
- Las macros externas y las DSL no allowlisted permanecen opacas; los aliases de macros estándar y sus expresiones anidadas se describen en [aliases](aliases.md).
- `async`/`await` está probado con `futures-executor`; no hay aliases/runtime propios y otros runtimes no están aún en la matriz. Macros externas se conservan opacas y las expande rustc; Óxido no expande ni traduce sus DSL. Auditoría de superficie detecta las formas descritas, no llamadas indirectas ni código expandido. Estos límites constan en la matriz y no amplían el alcance de Fase 8; Workspace Trust para compilaciones iniciadas por el LSP es trabajo de Fase 11. La CLI actual no ofrece sandbox para código de dependencias.

Las decisiones de estructura y dependencias están en [ADR 0004](adr/0004-estructura-indentada-rust.md) y [ADR 0005](adr/0005-dependencias-cargo-versionadas.md). La fase queda cerrada con esta matriz de alcance inicial. Workspace Trust corresponde al futuro LSP: no se deben iniciar compilaciones automáticas en workspaces no confiables. La CLI actual no sandboxa `build.rs` ni macros procedurales, por lo que el operador debe confiar el proyecto antes de resolver o compilar esas dependencias.
