# Óxido para desarrolladores

## Qué es hoy

Óxido es una herramienta de lenguaje en etapa temprana. `oxido-language` tokeniza archivos `.ox`, conserva spans UTF-8 y parsea un subconjunto estructural inicial. La CLI puede exportar, compilar y ejecutar `examples/hola_mundo.ox`. Las funciones y expresiones fuera de la matriz de compatibilidad todavía no están cubiertas.

Flujo disponible desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
cargo run --locked -p oxido-cli -- compilar examples/hola_mundo.ox
cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox
```

`exportar` crea además el mapa `.oxido-map.json`. `importar` acepta solo el subconjunto reversible de funciones libres y verifica que regenerar conserve el Rust formateado. `analizar-rust` sigue validando archivos `.rs` directamente.

## Dirección técnica

- Rust 2024 es la edición objetivo.
- La semántica de tipos, ownership y borrowing pertenece a Rust.
- El frontend Óxido transforma estructura y aliases; expresiones y tipos se conservan como Rust en el diseño inicial.
- Los argumentos de macros y árboles de atributos permanecen opacos.
- El backend genera con `quote`/`proc_macro2` y formatea con `prettyplease`; los spans se guardan en un mapa lateral.

Consulta la [especificación](/referencia/language-spec/), [arquitectura](/referencia/arquitectura/), [pipeline](/referencia/compiler-pipeline/) y [compatibilidad](/referencia/compatibility/).

## Contribuir

Trabaja desde una fase del [roadmap](/referencia/roadmap/), añade casos positivos y negativos, y documenta cualquier límite que aún no pueda probarse.
