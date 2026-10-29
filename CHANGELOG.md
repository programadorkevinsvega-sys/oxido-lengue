# Historial de cambios

## [Unreleased]

- Se reforzó la interoperabilidad con macros de Rust y la compilación reproducible con Cargo.
- La CI valida el MSRV declarado y ejecuta `cargo-deny`.
- Se publicó el modelo de amenazas y la matriz de límites de Fase 12; lexer y parser aceptan un plazo cooperativo de 10 s (`OX6001`) que el compilador, la CLI y el LSP aplican por fuente y por operación.
