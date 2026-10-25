# Fixture de crates.io para pruebas Cargo

Este fixture cubre una crate de formato numérico pura y una dependencia async real:

- `itoa` **1.0.18**, MIT/Apache-2.0.
- `futures-executor` **0.3.34**, MIT/Apache-2.0; sus transitivas también están fijadas en `Cargo.lock`.

El manifiesto y lockfile describen el paquete temporal `oxido_generated` que emite el compilador. El contenido de `vendor/` se generó con Cargo, no se editó manualmente:

```sh
cargo vendor --locked vendor
```

La prueba apunta un `CARGO_HOME` temporal a este directorio mediante `source.crates-io.replace-with`, de modo que no consulta la caché del desarrollador ni necesita red. Los checksums de los paquetes en `Cargo.lock` se comparan con `.cargo-checksum.json` antes de aceptar el resultado.

El fixture sirve para probar resolución, descarga offline, lockfile, checksum e interoperabilidad async. No representa crates con `build.rs` ni macros procedurales; esos riesgos tienen fixtures separados en `crates/oxido-cli/tests/cli.rs` y requieren confianza explícita al compilar.
