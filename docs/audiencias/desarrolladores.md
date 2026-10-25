# Óxido para desarrolladores

## Qué es hoy

Óxido es una herramienta de lenguaje en etapa temprana. El crate `oxido-language` tokeniza archivos `.ox`, conserva spans UTF-8 y parsea un subconjunto estructural inicial. La generación de Rust desde `.ox` y el uso de Cargo para compilar esos archivos todavía están pendientes.

El comando experimental disponible analiza sintaxis de archivos `.rs` para comprobar una parte del pipeline; no es un comando de compilación Óxido:

```sh
cargo run --locked -p oxido-cli -- analizar-rust ruta/al/archivo.rs
```

## Dirección técnica

- Rust 2024 es la edición objetivo.
- La semántica de tipos, ownership y borrowing pertenece a Rust.
- El frontend Óxido transforma estructura y aliases; expresiones y tipos se conservan como Rust en el diseño inicial.
- Los argumentos de macros y árboles de atributos permanecen opacos.
- La generación prevista usa tokens/AST, no concatenación de texto.

Consulta la [especificación](/referencia/language-spec/), [arquitectura](/referencia/arquitectura/), [pipeline](/referencia/compiler-pipeline/) y [compatibilidad](/referencia/compatibility/).

## Contribuir

Trabaja desde una fase del [roadmap](/referencia/roadmap/), añade casos positivos y negativos, y documenta cualquier límite que aún no pueda probarse.
