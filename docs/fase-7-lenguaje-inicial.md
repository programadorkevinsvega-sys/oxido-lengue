# Fase 7 — Lenguaje inicial

**Estado:** completada para el subconjunto ejecutable que se delimita en este informe. La mayoría de las expresiones, tipos y semántica siguen siendo Rust y las valida Cargo/rustc.

## Alcance implementado y probado

- Variables `let`/`dejar`, mutabilidad, literales, llamadas, operadores y expresiones Rust en el subconjunto del parser.
- Funciones, parámetros, tipos de retorno, visibilidad y valores de bloque.
- Condiciones y bucles estructurales cubiertos por Fase 6.
- Tuplas, arrays y tipos primitivos preservados como sintaxis Rust.
- `struct` con campos indentados; `enum` con variantes indentadas; en modo llaves se aceptan items Rust con llaves, incluidos métodos `impl` y módulos.
- Referencias y borrowing con la semántica estándar de Rust.
- Imports `use`/`usar`, `Result`, `Option`, operador `?` y colecciones estándar con nombres/métodos Rust.
- Pruebas end-to-end compilan y ejecutan [el ejemplo indentado](../examples/fase_7_estructuras.ox) y [el ejemplo de interoperabilidad](../examples/fase_7_interoperabilidad.ox). También hay un caso negativo para campos mal formados y matriz actualizada.

## Forma admitida para items indentados

Los campos y variantes usan líneas independientes, y el normalizador inserta las comas necesarias antes de validarlos con Syn. Los tipos y expresiones del interior conservan sintaxis Rust; los aliases de keyword se normalizan fuera de macros y atributos.

Las importaciones `usar` pueden omitir el punto y coma de Rust; el backend lo emite al generar el item.

```ox
sintaxis indentacion
estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)
```

`impl`, métodos y módulos se conservan como items Rust con llaves en esta versión. Todavía no se convierten sus cuerpos a bloques indentados. Tampoco hay brazos indentados para `match`, alias españoles para tipos/métodos de biblioteca, type checker propio ni runtime propio.

## Evidencia local

La fase se verifica con `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`. El build del portal publica el informe y los dos ejemplos desde Markdown/documentación del repositorio. La decisión del AST estructural está en [ADR 0003](adr/0003-items-indentados-iniciales.md); la compatibilidad exacta está en [la matriz](compatibility.md).
