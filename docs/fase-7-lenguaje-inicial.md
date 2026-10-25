# Fase 7 — Lenguaje inicial

**Estado:** completada para el subconjunto documentado y probado aquí. La mayoría de las expresiones, tipos y semántica siguen siendo Rust y las valida Syn y Cargo/rustc.

## Alcance implementado y probado

- Variables `let`/`dejar`/`variable`, `mutable` como alias individual de `mut` y pareja `variable mutable`, literales, llamadas, operadores y expresiones Rust en el subconjunto del parser.
- Funciones, parámetros, tipos de retorno, visibilidad y valores de bloque.
- Condiciones y bucles estructurales cubiertos por Fase 6.
- Tuplas, arrays y tipos primitivos preservados como sintaxis Rust.
- `struct` con campos indentados; `enum` con variantes indentadas; en modo llaves se aceptan items Rust con llaves, incluidos métodos `impl` y módulos.
- Referencias y borrowing con la semántica estándar de Rust.
- Imports `use`/`usar`, `Result`/`Resultado`, `Option`/`Opcion`, operador `?` y aliases iniciales de prelude descritos en [la tabla de aliases](aliases.md).
- `principal` genera la función Rust `main`; `Vector<Cadena>`, `&texto`, métodos comunes y macros allowlisted tienen aliases probados.
- Argumentos de macros opacos; solo se traduce una lista documentada de nombres de macros. `matches!` y macros de terceros permanecen sin cambios.
- Atributos allowlisted `#[prueba]`, `#[permitir(...)]` y `#[derivar(...)]` pasan generación y validación Cargo; las colisiones de alias del prelude sugieren escape `r#`.
- Un conjunto inicial de mensajes rustc se traduce a español desde eventos JSON sin perder código ni span; los códigos desconocidos conservan el mensaje original.
- Pruebas end-to-end compilan y ejecutan [el ejemplo indentado](../examples/fase_7_estructuras.ox) y [el ejemplo de interoperabilidad](../examples/fase_7_interoperabilidad.ox). También hay un caso negativo para campos mal formados y matriz actualizada.

## Forma admitida para items indentados

Los campos y variantes usan líneas independientes, y el normalizador inserta las comas necesarias antes de validarlos con Syn. Las expresiones/tipos siguen tokens Rust y aliases documentados. Los aliases no se traducen en argumentos de macros ni en atributos desconocidos.

Las importaciones `usar` pueden omitir el punto y coma de Rust; el backend lo emite al generar el item.

```ox
sintaxis indentacion
estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)
```

Al cerrar Fase 7, `impl`, métodos, traits y módulos aún se escribían con llaves Rust, y `match` no tenía brazos indentados. La [primera subfase de Fase 8](fase-8-integracion-rust.md) ya añade estas formas estructurales. Siguen fuera de alcance un traductor completo de mensajes de error, un type checker propio y un runtime propio. Los diagnósticos Cargo traducen un conjunto inicial de códigos comunes y conservan código, span y detalle JSON.

## Evidencia local

Verificación ejecutada: `cargo fmt --all`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` y `npm run build` en `website/`. Los ejemplos compilan/ejecutan con Cargo; el build del portal publica el informe desde Markdown. La decisión del AST estructural está en [ADR 0003](adr/0003-items-indentados-iniciales.md); la compatibilidad exacta está en [la matriz](compatibility.md).
