# Fase 2: infraestructura del monorepo

**Estado:** completada para el alcance de infraestructura inicial. La sintaxis `.ox`, el lexer Óxido y la integración de compilación Cargo quedan en las fases siguientes.

Este informe registra la infraestructura entregada en Fase 2; las verificaciones al cerrar fases posteriores también vuelven a ejecutar el workspace completo.

## Entregado

- Workspace Cargo con resolver 3, edición 2024, MSRV 1.85, lockfile versionado y toolchain fijada en 1.99.1.
- Cuatro crates con responsabilidades reales: dominio de fuente, backend Rust, aplicación y CLI.
- Dirección de dependencias sin ciclos; `syn` y `prettyplease` quedan en el backend.
- Parser de `oxido.toml` como datos TOML declarativos: solo campos admitidos, sin comandos, modo opcional, edición 2024 y máximo de 64 KiB.
- `#![forbid(unsafe_code)]`, formato, Clippy y CI con `--locked`.
- Pipeline inicial Rust: leer fuente → parsear AST → formatear → volver a parsear.
- Pruebas de integración para Rust válido e inválido, configuración válida, campos desconocidos, edición no soportada y límite de tamaño.

## Límite explícito

El pipeline acepta Rust como entrada de comprobación. No implementa lexer/parser de Óxido, transformación `.ox → .rs`, descubrimiento/carga automática de `oxido.toml`, invocación de Cargo, LSP ni extensión VS Code. Esas capacidades no se presentan como existentes.

## Evidencia

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all -- --check`
- El binario `oxido analizar-rust` se comprueba contra `crates/oxido-cli/src/main.rs` y reporta cuatro items Rust válidos.
- En la verificación histórica de esta fase, GitHub no inició los jobs por un límite de facturación. Estado vigente: el repositorio es público y la CI remota está activa; cada cambio debe revisarse contra los checks actuales del PR.
