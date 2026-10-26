# Contribuir a Óxido

Lee [AGENTS.md](AGENTS.md), [ARCHITECTURE_RULES.md](ARCHITECTURE_RULES.md) y la especificación en `docs/` antes de cambiar el compilador.

Antes de abrir un cambio ejecuta:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
```

Los cambios deben conservar la semántica de Rust, incluir regresiones para el comportamiento nuevo y no introducir `unsafe` ni silenciamiento de resultados en el código interno.
