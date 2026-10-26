# Fase 8 — Integración con Rust

**Estado:** completada para el corpus de interoperabilidad verificado y la implementación actual. No declara compatibilidad general ni limita permanentemente el lenguaje. La meta normativa es interoperar con bibliotecas Rust sin exigir cambios en su fuente.

## Contrato de interoperabilidad

Óxido analiza y traduce su sintaxis estructural; Rust conserva la semántica de expresiones, tipos, traits, macros, asincronía y `unsafe` que el frontend preserva. Cargo/rustc resuelve dependencias, expande macros, valida tipos y compila. Preservar significa conservar tokens, delimitadores, nombres y estructura con significado para Rust, no copiar texto sin análisis. Si una construcción no puede preservarse fielmente ni transformarse mediante una regla definida, Óxido debe diagnosticarla en vez de alterarla silenciosamente.

Las macros de bibliotecas deben funcionar sin modificar la biblioteca. Admitirlas significa preservar las invocaciones, atributos y árboles de tokens para que Cargo/rustc los procese; no significa implementar un sistema Óxido para definir, analizar o expandir macros Rust. `macro_rules!` y macros locales escritas como Rust pasan por el frontend como Rust. Los aliases allowlisted se traducen con sus reglas explícitas y las DSL externas permanecen opacas. `async/await` usa el runtime elegido por la aplicación. `unsafe` conserva su semántica y solo una opción explícita activa el rechazo.

## Cobertura actual y brechas de conformidad

| Área | Evidencia registrada | Límite explícito |
|---|---|---|
| Crates externas | E2E con `itoa`, `futures-executor`, `serde`, `serde_json`, Tokio y crates locales, offline y bajo lockfile | Solo están demostradas las versiones y construcciones del fixture |
| Traits, genéricos y lifetimes | Corpus E2E con bounds, tipos asociados, `impl Fn`, closure e iteradores | Aumentar casos de traits/genéricos y versiones de crates sin reescribir sus fuentes |
| Macros declarativas y DSL | E2E ejecuta `macro_rules!`, `serde_json::json!` 1.0.154, `tokio::select!` 1.54.1 y `paste::paste!` 1.0.15; aliases `si`/`tipo`/`verdadero`, cadenas y grupos anidados llegan sin traducirse dentro de las DSL | No se implementa un expansor Óxido; otras macros/versiones requieren pruebas propias |
| Macros procedurales | E2E offline ejecuta derive/atributo de Serde 1.0.229 y `tokio::main`; `paste!` recibe `[<macro_ si>]`, genera una función usada y ejecutada | Compatibilidad demostrada solo para los casos y versiones listados |
| `async/await` | Future con `.await` ejecutado mediante `futures-executor`; macros de Tokio probadas en la suite de interoperabilidad | El runtime pertenece al proyecto Rust y no lo proporciona Óxido |
| `unsafe` | Auditoría estática acotada y opción explícita de rechazo; una regresión compila una API de puntero válida y la auditoría por defecto no bloquea | La auditoría no ve llamadas indirectas ni expansiones |
| Diagnósticos | Cargo JSON y mapas por módulo relacionan errores con `.ox` cuando existe correspondencia; caso negativo de macro distingue `principal.ox` de `src/main.rs` exportado | Cargo no consume los mapas laterales al compilar el árbol exportado |

La cobertura adicional debe añadirse como nuevas regresiones identificables; no extiende automáticamente los resultados de esta matriz. Ningún caso se considera aprobado sin ejecución reproducible.

## Subfase 1 — Estructura indentada

**Estado:** completada y verificada localmente.

La sintaxis indentada inicial admite:

- `implementar Tipo:` con métodos, métodos asociados y items asociados Rust terminados en `;`.
- `rasgo Nombre:` con declaraciones de métodos terminadas en `;`, métodos por defecto y items asociados.
- `modulo nombre:` con items anidados recursivamente.
- `coincidir expresion:` como expresión independiente o inicializador `dejar x = coincidir ...:`.
- Brazos `patron => expresion` y `patron =>` seguido de un bloque indentado.

Los encabezados, patrones, tipos y expresiones siguen tokens Rust con aliases Óxido documentados. Los argumentos de macros estándar que reciben expresiones pueden normalizar aliases anidados; las DSL externas y macros no allowlisted conservan sus tokens opacos. Syn valida cada item/contexto y Cargo/rustc mantiene la validación semántica.

Ejemplo combinado: [fase_8_estructura_indentada.ox](../examples/fase_8_estructura_indentada.ox). Se ejecuta con `cargo run --locked -p oxido-cli -- ejecutar examples/fase_8_estructura_indentada.ox`.

Validación registrada: el ejemplo se compila y ejecuta mediante Cargo; el cierre más reciente ejecutó `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` y `cargo test --workspace --locked`. Estos resultados corresponden a los comandos ejecutados en ese cambio, no a cada revisión futura.

La cobertura de regresión de los bloques `implementar` verifica receivers `mismo`, `&mismo` y `&mutable mismo`; funciones asociadas sin receiver; genéricos y `donde` junto al `:` que abre el bloque; varios bloques para el mismo tipo; visibilidad `publico`; atributos y comentarios `///`; constantes asociadas y tipos asociados en traits/implementaciones. También comprueba el rechazo de bloques `implementar` vacíos, la mala indentación, la regla del `;` final, el mismo resultado al compilar formas indentadas y con llaves, y el mapeo de errores de rustc a la línea original `.ox`.

La regresión de documentación compara el Rust generado para comentarios `///` antes del primer, intermedio y último método; `///` con líneas en blanco y `#[inline]`; bloques `/** ... */`; comentarios internos `//!`/`/*! ... */`; comentarios ordinarios al cerrar el bloque y comentarios al final de su encabezado. La prueba CLI de Fase 5 confirma que la documentación se conserva semánticamente como `doc` y que comentarios ordinarios se mantienen en la ida y vuelta fuera de grupos opacos.

## Subfases 3 y 4 — Genéricos, lifetimes, patrones, closures e iteradores

**Estado:** verificadas para el corpus descrito; no implican compatibilidad general con Rust.

`compiles_generics_lifetimes_closures_iterators_and_patterns_in_both_modes` ejecuta el mismo programa en modo indentación y llaves. Usa una función genérica con lifetime explícito y bound `where`, un parámetro `impl Fn`, `Iterator::find`, una closure, patrones `Some`/`None` y `coincidir`. Ambos programas producen `Ana`. La prueba confirma generación y compilación con Cargo/rustc, además de igualdad observable entre los dos modos.

Los tipos, expresiones, closures y patrones dentro de estas firmas y cuerpos siguen siendo sintaxis Rust validada por Syn y semántica delegada a rustc. La inferencia local, cuando Rust puede resolverla, es estática y corresponde a `rustc`; Óxido no infiere firmas ni implementa inferencia propia. La cobertura de lifetimes, traits, patrones e iteradores se limita al corpus registrado en la matriz.

## Subfase 2 — Dependencias Cargo

**Estado:** verificada para el alcance de fuentes `path` y registry de crates.io.

`oxido.toml` admite una dependencia `[dependencies.nombre]` con exactamente `path` relativo al proyecto o `version` SemVer de crates.io; también permite `features` y `default-features`. Se rechazan URLs/git, opciones desconocidas, requisitos SemVer inválidos y mezclar ambas fuentes. Las rutas locales se canonicalizan y se rechazan escapes, incluidos enlaces simbólicos.

`oxido descargar-dependencias archivo.ox` muestra el cambio de lockfile y paquetes de crates.io; `--confirmar` aplica el plan mediante `cargo fetch --locked`, sin compilar ni ejecutar código. `--actualizar` también requiere confirmación. Fuentes `path` requieren `--permitir-rutas-locales`; git/URL no se soportan. `compilar` y `ejecutar` usan `--locked --offline`, devuelven `OX7001` si falta el lockfile para cualquier dependencia y requieren `--confiar-dependencias` para compilar cualquier dependencia. Las pruebas demuestran que fetch no activa build scripts ni macros procedurales y que la compilación confiada sí los activa. Pruebas negativas cambian un archivo vendorizado y un checksum del lockfile; Cargo falla en ambos casos. El E2E compara `.rs` emitido desde dos directorios y comprueba que no contiene rutas absolutas. La procedencia y regeneración del vendor están en [README del fixture](../tests/fixtures/cargo-registry/README.md), y la política en [ADR 0006](adr/0006-descarga-y-confianza-de-dependencias.md).

Cargo ejecuta `build.rs` y macros procedurales durante compilación, no durante `cargo fetch`. `--offline` evita descargas, no aísla procesos. La CLI exige `--confiar-dependencias`; el flag no es un sandbox. El LSP inicial no inicia Cargo; cualquier compilación iniciada desde el editor requiere Workspace Trust.

## Subfase 5 — Macros declarativas y atributos

**Estado:** verificada para los casos descritos, con delegación de expansión a Rust.

La prueba CLI `interoperates_with_registry_proc_macros_in_direct_and_exported_module_trees` usa dependencias vendorizadas y un `CARGO_HOME` temporal sin caché de crates. Ejecuta `#[derive(serde::Serialize, serde::Deserialize)]`, `#[serde(rename = ...)]`, `#[tokio::main(flavor = "current_thread")]`, `tokio::select!`, `serde_json::json!` y `paste::paste!`; también una macro `macro_rules!`. `paste!` consume `[<macro_ si>]`, genera `macro_si`, y el programa llama y ejecuta esa función. El cuerpo generado usa `stringify!` con identificadores, aliases de Óxido, una cadena y delimitadores anidados. Un módulo `.ox` apunta mediante `#[path]` a `.rs`. El parser conserva la invocación multilínea con llaves como un fragmento único. La prueba compila y ejecuta mediante Óxido; después copia el árbol exportado a un directorio hermano independiente, elimina los mapas laterales, configura su propio `CARGO_TARGET_DIR` y compila/ejecuta allí con `cargo --locked --offline`. No depende de fuentes ni artefactos del proyecto original y la salida coincide.

El caso negativo usa `serde_json::json!(@)`. `oxido compilar` informa `principal.ox` y una ubicación; Cargo sobre la exportación independiente informa `src/main.rs` y una ubicación generada, pues Cargo no lee los mapas laterales de Óxido. No se presenta el diagnóstico directo de Cargo como si conservara el nombre `.ox`.

Los comentarios ordinarios dentro de argumentos de macros o árboles de atributos opacos siguen rechazándose con `OX4001`, porque `syn`/`TokenStream` no los conservan. Regresiones comprueban ambos contextos; no se descartan ni se reubican silenciosamente.

Óxido no implementa un sistema propio de macros Rust. Las definiciones e invocaciones escritas en Rust se preservan como tokens; Cargo/rustc resuelve dependencias, expande macros y compila. Las macros externas probadas no requieren cambios en sus fuentes. Los aliases estándar solo se traducen según su allowlist y las DSL externas se mantienen opacas. Estos resultados cubren versiones concretas fijadas en [el fixture de crates](../tests/fixtures/cargo-registry/README.md), no cualquier macro ni cualquier versión. El atributo `#[path]` y el árbol mixto quedan demostrados con la estructura del fixture.

Las verificaciones obligatorias del workspace son:

```sh
cargo fmt --all -- --check
cargo check --locked --offline --workspace
cargo test --locked --offline --workspace
```

La regresión E2E de macros se puede repetir por separado:

```sh
cargo test --locked --offline -p oxido-cli interoperates_with_registry_proc_macros_in_direct_and_exported_module_trees -- --nocapture
cargo test --locked --offline -p oxido-cli dependency_fetch_never_runs_build_scripts_or_proc_macros_without_explicit_trust -- --nocapture
```

Las pruebas crean un `CARGO_HOME` temporal que solo apunta al directorio vendorizado. `--locked --offline` prueba repetibilidad con esas dependencias ya disponibles localmente; no prueba descarga sin red. Para regenerar el fixture desde el manifiesto, sigue los comandos `cargo generate-lockfile` y `cargo vendor --locked vendor` en su [README](../tests/fixtures/cargo-registry/README.md), revisa el diff del lockfile y valida checksums con la suite CLI.

## Subfase 6 — Async/await y concurrencia

**Estado:** verificada para futures inmediatamente listos, Tokio en la matriz de macros y concurrencia estándar; sin runtime propio.

La prueba E2E con la dependencia real `futures-executor` vendorizada ejecuta un future que usa `.await` y compara la salida. También valida el Rust generado (`async fn`, `.await`) con rustc; la tarea `std::thread::spawn` ejecuta y comprueba su resultado. Óxido no incorpora runtime async, executor, scheduler ni aliases de concurrencia: el proyecto elige la biblioteca/runtime y la semántica de suspensión sigue delegada a esa dependencia.

## Subfase 8 — Política y auditoría de `unsafe`

**Estado:** verificada para el subconjunto detectable documentado.

`oxido auditar-unsafe <archivo.ox|archivo.rs> [--rechazar]` muestra posición de cada token y puede rechazarlo sin compilar. `oxido auditar-proyecto <directorio>` enumera tokens `unsafe`/`inseguro`, archivos `build.rs`, manifiestos que declaran crates `proc-macro`, bloques FFI y llamadas léxicas `Command::new`. Omite directorios de build comunes, no sigue symlinks, limita el número de entradas y muestra rutas relativas para no filtrar rutas absolutas.

El informe es estático y conservador: no resuelve todas las dependencias registry ni relaciones indirectas con macros procedurales, puede contar código de macros opacas, no ve código generado por expansión y solo reconoce la forma visible `Command::new`. No prueba ausencia de riesgo ni reemplaza sandbox, Workspace Trust o revisión de dependencias.

## Subfase 9 — Matriz por edición y versiones

**Estado:** completada para la edición declarada y el corpus probado.

La matriz de [compatibilidad](compatibility.md) declara Rust 2024, MSRV 1.99.1 y las dependencias E2E verificadas. No implica compatibilidad con todas las versiones de Rust o crates; `Cargo.lock` y las pruebas por característica son la evidencia reproducible.

## Límites de esta entrega

- Los rasgos e implementaciones conservan las reglas de Rust para receivers, coherencia, tipos asociados y métodos requeridos.
- Los brazos conservan sintaxis Rust en patrones, guards y expresiones; solo su estructura externa usa indentación.
- Las macros externas y las DSL probadas se preservan opacas; los aliases estándar y las expresiones que traducen se describen en [aliases](aliases.md). No se afirma fidelidad universal para toda DSL, macro o versión de Rust.
- `async/await` está probado con `futures-executor` y la matriz de macros incluye Tokio; no hay runtime propio, porque la aplicación selecciona su dependencia. La auditoría de `unsafe` solo detecta las formas descritas; no prueba ausencia de riesgo. El LSP inicial ofrece transporte y diagnósticos léxicos básicos; su respuesta de semantic tokens es heurística e incompleta. Workspace Trust y compilación desde el editor no están implementados. La CLI no ofrece sandbox para código de dependencias.

Las decisiones de estructura, dependencias e interoperabilidad están en [ADR 0004](adr/0004-estructura-indentada-rust.md), [ADR 0005](adr/0005-dependencias-cargo-versionadas.md) y [ADR 0007](adr/0007-contrato-interoperabilidad-rust.md). La Fase 8 está completada para el corpus verificado; ampliar compatibilidad requiere nuevas pruebas y no cambia los límites actuales. Workspace Trust debe impedir compilaciones automáticas en workspaces no confiables. La CLI no sandboxa `build.rs` ni macros procedurales, por lo que el operador debe confiar el proyecto antes de resolver o compilar esas dependencias.
