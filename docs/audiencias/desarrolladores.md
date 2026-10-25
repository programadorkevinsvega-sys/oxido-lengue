# Óxido para desarrolladores

## Qué es hoy

Óxido es una herramienta de lenguaje en etapa temprana. `oxido-language` tokeniza y parsea un subconjunto estructural; la CLI exporta, compila y ejecuta ejemplos reales. Fases 7–8 incluyen items indentados, aliases de prelude y macros estándar, además de interoperabilidad Rust en módulos, métodos, referencias, `Result`/`Option`, `?` y colecciones. Los límites por característica están en la matriz de compatibilidad.

Flujo disponible desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_estructuras.ox
cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_interoperabilidad.ox
```

`exportar` crea además el mapa `.oxido-map.json`. `importar` acepta solo el subconjunto reversible de funciones libres y verifica que regenerar conserve el Rust formateado. `analizar-rust` sigue validando archivos `.rs` directamente.

La CLI admite dependencias Cargo locales dentro del proyecto mediante `oxido.toml`:

```toml
[language]
syntax = "indentation"
edition = "2024"

[dependencies.mi_crate]
path = "vendor/mi_crate"
```

`compilar` y `ejecutar` resuelven esas rutas y usan Cargo offline. Cargo puede ejecutar `build.rs` o macros procedurales de la dependencia cuando se invoca explícitamente. Dependencias de registro/git y un lockfile persistente siguen pendientes.

## Dirección técnica

- Rust 2024 es la edición objetivo.
- La semántica de tipos, ownership y borrowing pertenece a Rust.
- El frontend Óxido transforma estructura y aliases; tipos y expresiones conservan sintaxis Rust.
- Las macros estándar allowlisted traducen su nombre y pueden normalizar aliases en expresiones anidadas; DSLs de macros externos/no allowlisted y árboles de atributos desconocidos permanecen opacos.
- El backend genera con `quote`/`proc_macro2` y formatea con `prettyplease`; los spans se guardan en un mapa lateral.

Consulta la [especificación](/referencia/language-spec/), [arquitectura](/referencia/arquitectura/), [pipeline](/referencia/compiler-pipeline/) y [compatibilidad](/referencia/compatibility/).

## Contribuir

Trabaja desde una fase del [roadmap](/referencia/roadmap/), añade casos positivos y negativos, y documenta cualquier límite que aún no pueda probarse.
