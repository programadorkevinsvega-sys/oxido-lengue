# Proyecto Óxido de pruebas CRUD y concurrencia

Este directorio contiene un proyecto de ejemplo en Óxido: `oxido.toml`, `Cargo.lock`, la API `crud_api_concurrencia.ox` y el cliente de pruebas `verificar_api_concurrencia.ox`. La API usa Axum y Tokio; el cliente está escrito en Óxido y usa `TcpStream` y threads de Rust para generar solicitudes simultáneas sin agregar otra dependencia HTTP.

## Ejecutar

Desde la raíz del repositorio, en dos terminales:

```sh
cargo run --locked -p oxido-cli -- ejecutar prueba/crud_api_concurrencia.ox --confiar-dependencias
```

```sh
cargo run --locked -p oxido-cli -- ejecutar prueba/verificar_api_concurrencia.ox --confiar-dependencias
```

El cliente verifica JSON inválido, precios no finitos y negativos, nombres vacíos y CRUD; además realiza dos rondas de lotes de 100, 500 y 1.000 altas. Cada ola admite hasta 100 solicitudes simultáneas para mantener estable la regresión funcional; verifica IDs únicos/consecutivos y recuento total. Esto no mide rendimiento ni acredita 1.000 conexiones concurrentes.

Para comprobar el desbordamiento de `u32`, reinicia la API con `CRUD_ID_INICIAL=4294967295`. Deja esa API ejecutándose en una terminal y, en otra, ejecuta el cliente en modo `overflow`:

```sh
CRUD_ID_INICIAL=4294967295 cargo run --locked -p oxido-cli -- ejecutar prueba/crud_api_concurrencia.ox --confiar-dependencias
```

```sh
CRUD_TEST_MODE=overflow cargo run --locked -p oxido-cli -- ejecutar prueba/verificar_api_concurrencia.ox --confiar-dependencias
```

La API debe responder `409 Conflict` y mantener vacío el almacén. La dirección por defecto es `127.0.0.1:38211`; puede cambiarse con `CRUD_ADDR` y `CRUD_TEST_ADDR`.

Las dependencias deben descargarse explícitamente con `oxido descargar-dependencias` y revisarse antes de confirmar. `--confiar-dependencias` autoriza compilar crates, lo que permite ejecutar sus `build.rs` y macros procedurales.

## Interoperabilidad con la biblioteca estándar

`interoperabilidad_std/` es un proyecto Óxido independiente, con su propio `oxido.toml` y sin dependencias de terceros. Además de comprobar que compila, ejecuta un pequeño runner de pruebas hecho en Óxido:

```sh
cargo run --locked -p oxido-cli -- ejecutar prueba/interoperabilidad_std/main.ox
```

Comprueba `BTreeMap`, `HashSet` y `VecDeque`; archivos temporales y `Cursor`; rutas con `PathBuf`; threads, `Arc`, `Mutex` y canales; y medición de tiempo. Falla el proceso si una aserción no se cumple. Se conserva junto a `crud_api_concurrencia.ox` y `verificar_api_concurrencia.ox` para mantener regresiones distintas: aplicación HTTP concurrente y herramienta de pruebas local basada en `std`.

El proyecto independiente [`crud_sqlite/`](crud_sqlite/README.md) amplía la regresión HTTP con SQLx y verifica persistencia real al reiniciar el servidor. [`crud_sqlite_rust/`](crud_sqlite_rust/README.md) ofrece una línea base 100 % Rust y compara ambos servidores con el mismo binario cliente, archivo SQLite, dependencias y máquina.

El job `reference-regressions` de CI ejecuta automáticamente interoperabilidad con `std`, CRUD concurrente (incluido el desbordamiento de ID), una comprobación HTTP secuencial de los servidores Óxido/Rust y la prueba de persistencia del proyecto exportado. El supervisor Rust está en `tools/validation-report/src/bin/reference-runner.rs`; usa el puerto 38211 y procesos temporales del runner. Estas regresiones funcionales no calculan rendimiento. El benchmark comparativo release se ejecuta semanalmente y puede iniciarse también desde `workflow_dispatch`; esta actualización no ejecutó ni midió el benchmark.

[`crud_sqlite_export_rust/`](crud_sqlite_export_rust/README.md) conserva las salidas `.rs` de la API y del verificador SQLite generadas por `oxido exportar`. Compila ambos binarios y ejecuta el cliente Rust exportado a través de reinicios reales para validar el ciclo completo `.ox` → `.rs` → compilación y ejecución.

[`modulos_jerarquicos/`](modulos_jerarquicos/README.md) prueba módulos Óxido distribuidos en archivos y subdirectorios. La exportación individual, compilación y ejecución funcionan; la importación de declaraciones/módulos y la compilación directa de un árbol de archivos `.ox` quedan documentadas como límites actuales.

[`terminal-cli-oxido-py/`](terminal-cli-oxido-py/README.md) conserva la jerarquía del CLI de terminal y convierte sus fuentes a `.ox` con sintaxis indentada y keywords en español. Incluye instrucciones para exportarlo, compilar el binario nativo interactivo y ejecutar los tests que ya tenía el parser.
