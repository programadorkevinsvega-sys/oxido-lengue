# Exportación Rust de la prueba SQLite de Óxido

Este proyecto contiene las salidas generadas por `oxido exportar` para:

- `../crud_sqlite/crud_api_persistencia.ox` → `src/main.rs`.
- `../crud_sqlite/verificar_persistencia.ox` → `src/bin/verificar_persistencia.rs`.

Los `.ox` son la fuente de verdad. Para regenerar desde la raíz del repositorio:

```sh
cargo run --locked -p oxido-cli -- exportar prueba/crud_sqlite/crud_api_persistencia.ox --salida prueba/crud_sqlite_export_rust/src/main.rs
cargo run --locked -p oxido-cli -- exportar prueba/crud_sqlite/verificar_persistencia.ox --salida prueba/crud_sqlite_export_rust/src/bin/verificar_persistencia.rs
```

El exportador también crea mapas `.oxido-map.json`. Se pueden retirar después de revisar los mapeos; Cargo no los necesita.

Compila ambos binarios en perfil optimizado, usando las mismas dependencias y features que el proyecto `.ox`:

```sh
CARGO_TARGET_DIR=/tmp/crud-sqlite-export-target cargo build --release --locked --offline --manifest-path prueba/crud_sqlite_export_rust/Cargo.toml --bins
```

La prueba de ida, reinicio, mutaciones y vuelta se ejecuta con:

```sh
CARGO_TARGET_DIR=/tmp/crud-sqlite-export-target python3 prueba/crud_sqlite_export_rust/verificar_exportacion.py
```

Este script de orquestación conserva la separación de procesos y reinicios del servidor. Usa Python estándar solo como lanzador de la regresión; los binarios verificados son Rust exportado y se compilan con Cargo en `--release`.

Comprueba que el servidor exportado conserva SQLite al reiniciarse y que el cliente exportado puede realizar las altas concurrentes y validar las mutaciones. El 25-10-2026 se compilaron ambos binarios con `--release --locked --offline` y pasó el flujo completo: inserción inicial, reinicio con persistencia, 40 altas concurrentes con IDs únicos, conflicto UNIQUE sin cambio de filas, actualización/eliminación y verificación tras otro reinicio.
