# Historial de cambios

## [Unreleased]

- Se reforzó la interoperabilidad con macros de Rust y la compilación reproducible con Cargo.
- La CI valida el MSRV declarado y ejecuta `cargo-deny`.
- Se publicó el modelo de amenazas y la matriz de límites de Fase 12; lexer y parser aceptan un plazo cooperativo de 10 s (`OX6001`) que el compilador, la CLI y el LSP aplican por fuente y por operación.
- `#![forbid(unsafe_code)]` cubre los seis crates y el binario del LSP; `oxido auditar-proyecto` acepta `--rechazar` para fallar ante usos detectados.
- La detección de caracteres confundibles incorpora alfabetos armenios, formas y dígitos de ancho completo.
- El índice del LSP restringe la caché a `0700`/`0600` en Unix y no la publica si falla la restricción de permisos.
- Lexer, parser y compilador limitan a 1.000 diagnósticos por fuente y resumen los omitidos con `OX6001`.
- Se integró `cargo-audit` 0.22.2 en CI y se documentó la política de dependencias y licencias en `docs/politica-dependencias.md`.
