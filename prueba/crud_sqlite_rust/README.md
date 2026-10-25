# Benchmark equivalente 100 % Rust

Este proyecto es una línea base independiente escrita íntegramente en Rust: servidor Axum/SQLx/SQLite en `src/main.rs` y cliente concurrente en `src/bin/benchmark.rs`. Usa las mismas versiones y features, esquema, rutas, pool de 8 conexiones, timeout SQLite de 10 s, carga HTTP/1 por `TcpStream`, tamaños 50/100/200, dos repeticiones y métricas que `../crud_sqlite/`.

El lockfile se genera con Cargo y las ejecuciones siguientes usan `--locked`. La compilación ocurre antes de la medición; el cronómetro del cliente empieza dentro del binario.

```sh
CARGO_TARGET_DIR=/tmp/crud-sqlite-rust-target cargo build --release --locked --manifest-path prueba/crud_sqlite_rust/Cargo.toml --bins
```

Inicia el servidor con una base nueva:

```sh
CRUD_SQLITE_PATH=/tmp/crud-sqlite-rust-benchmark.sqlite CARGO_TARGET_DIR=/tmp/crud-sqlite-rust-target /tmp/crud-sqlite-rust-target/release/crud-api-rust
```

En otra terminal ejecuta el mismo perfil de carga:

```sh
CARGO_TARGET_DIR=/tmp/crud-sqlite-rust-target /tmp/crud-sqlite-rust-target/release/benchmark-rust
```

## Comparación controlada de servidores

Ejecuta el supervisor secuencial desde la raíz del repositorio:

```sh
python3 prueba/crud_sqlite_rust/benchmark_secuencial.py --rondas 5
```

El script compila el CLI y el servidor/cliente Rust con `--locked --offline`, compila primero el servidor generado por Óxido con `--release` y lo inicia una vez para sacar la compilación del periodo medido. Comprueba que el proceso de Óxido ejecuta un binario ubicado en `target/release`, calienta ambos servidores con diez consultas GET, y luego mide cinco pares alternando el orden. Tras compilar, mide directamente el mismo ejecutable generado `release/oxido_generated`; así el proceso Cargo/CLI no forma parte de las rondas. En cada par inicia Óxido, ejecuta **el mismo binario `benchmark-rust` release**, detiene Óxido y comprueba que el puerto quedó libre; solo entonces inicia y mide Rust nativo. Si el puerto ya estaba ocupado, el script se niega a empezar.

Ambos usan la misma ruta SQLite, vaciada al arrancar cada servidor. Los lockfiles tienen el mismo grafo, versiones y checksums de dependencias de registro; solo cambia el nombre del paquete raíz. La compilación queda fuera de las mediciones. Cada invocación del cliente reporta dos repeticiones por nivel de carga (50, 100 y 200), QPS, p50/p95/p99, errores, ticks de CPU y RSS muestreado.

No ejecutes manualmente los dos servidores a la vez: comparten puerto y archivo SQLite. Para que la comparación sea limpia, cierra otros procesos que usen el puerto 38212 y evita carga concurrente en la máquina.

La medición histórica inicial y sus límites están registrados en [`RESULTADOS.md`](RESULTADOS.md); se hizo en perfil debug y no sirve para afirmar rendimiento release.

Los datos de `/proc` son específicos de Linux. Este benchmark exploratorio no prueba rendimiento de producción ni compatibilidad general; sus resultados son sensibles al hardware, al sistema operativo, al estado de SQLite y a la carga de la máquina.
