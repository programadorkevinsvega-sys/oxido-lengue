# Fase 2: infraestructura del monorepo

**Estado:** completada para el alcance de infraestructura inicial. La sintaxis `.ox`, el lexer Óxido y la integración de compilación Cargo quedan en las fases siguientes.

## Entregado

- Workspace Cargo con resolver 3, edición 2024, MSRV 1.85, lockfile versionado y toolchain fijada en 1.99.1.
- Cuatro crates con responsabilidades reales: dominio de fuente, backend Rust, aplicación y CLI.
- Dirección de dependencias sin ciclos; `syn` y `prettyplease` quedan en el backend.
- Parser de `oxido.toml` como datos TOML declarativos: solo campos admitidos, sin comandos, modo opcional y edición 2024.
- `#![forbid(unsafe_code)]`, formato, Clippy y CI con `--locked`.
- Pipeline inicial Rust: leer fuente → parsear AST → formatear → volver a parsear.
- Pruebas de integración para Rust válido e inválido, configuración válida, campos desconocidos y edición no soportada.

## Límite explícito

El pipeline acepta Rust como entrada de comprobación. No implementa lexer/parser de Óxido, transformación `.ox → .rs`, descubrimiento/carga automática de `oxido.toml`, invocación de Cargo, LSP ni extensión VS Code. Esas capacidades no se presentan como existentes.

## Evidencia

- `cargo check --workspace`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all -- --check`
- El binario `oxido analizar-rust` se comprueba contra una fuente Rust del propio workspace.
