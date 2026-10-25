# CRUD persistente con Óxido, Axum y SQLite

Este es un proyecto de prueba independiente. El servidor y el cliente de regresión están escritos en `.ox`; el backend usa Axum, Serde, Tokio y SQLx 0.8.6 con SQLite. Las consultas son dinámicas de SQLx: no requieren macros de consulta ni una base instalada por separado. El manifiesto y `Cargo.lock` fijan las dependencias.

## Preparar dependencias

La descarga usa el comando explícito de Óxido; no compila crates:

```sh
cargo run --locked -p oxido-cli -- descargar-dependencias prueba/crud_sqlite/crud_api_persistencia.ox --confirmar
```

Después de revisar las dependencias, `--confiar-dependencias` permite que Cargo compile SQLx, sus dependencias y sus macros de compilación.

## Flujo de regresión

Usa dos terminales. En cada reinicio conserva exactamente el mismo `CRUD_SQLITE_PATH`. El ejemplo guarda la base bajo `/tmp`; elimina la base y el checkpoint antes de iniciar una corrida nueva:

```sh
rm -f /tmp/oxido-crud-persistencia-test.sqlite /tmp/oxido-crud-persistencia-test.sqlite-shm /tmp/oxido-crud-persistencia-test.sqlite-wal
rm -f /tmp/oxido-crud-sqlite-instance.txt
```

Terminal 1, inicia el servidor:

```sh
CRUD_SQLITE_PATH=/tmp/oxido-crud-persistencia-test.sqlite cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/crud_api_persistencia.ox --confiar-dependencias
```

Después de interrumpir `oxido ejecutar`, verifica que el puerto quedó libre antes de reiniciar. En Linux, `ss -ltnp | grep ':38212'` muestra el PID que aún escucha; si el proceso hijo sigue activo, termínalo con `kill -TERM <PID>`.

Terminal 2, crea un producto y comprueba la primera respuesta:

```sh
CRUD_TEST_MODE=preparar cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/verificar_persistencia.ox --confiar-dependencias
```

El modo `preparar` guarda el identificador de arranque del servidor en un checkpoint. Detén el servidor, vuelve a iniciarlo con el mismo comando y comprueba que el producto siga allí. `reinicio` falla si el identificador no cambió, así que cambiar solo el modo del cliente no pasa la prueba:

```sh
CRUD_TEST_MODE=reinicio cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/verificar_persistencia.ox --confiar-dependencias
```

Sin detener el servidor, ejecuta 40 altas concurrentes, una violación SQL `UNIQUE`, una actualización y una eliminación:

```sh
CRUD_TEST_MODE=mutaciones cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/verificar_persistencia.ox --confiar-dependencias
```

Detén y reinicia otra vez el servidor con la misma ruta de base de datos. `final` exige otro identificador de arranque y comprueba que la actualización y la eliminación persistieron:

```sh
CRUD_TEST_MODE=final cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/verificar_persistencia.ox --confiar-dependencias
```

El servidor usa transacciones para insertar: confirma la fila al tener éxito y revierte ante el error de base de datos. Los errores de restricción única responden `409`; los demás errores SQL se convierten en `500` sin exponer detalles internos. El cliente verifica el total de filas antes y después del error.

## Benchmark funcional local

Con el servidor en ejecución, el modo `benchmark` repite dos veces cargas concurrentes de 50, 100 y 200 altas. Reporta solicitudes exitosas/errores, solicitudes exitosas por segundo y latencia p50/p95/p99. En Linux también lee los ticks de CPU y muestrea el RSS del proceso servidor desde `/proc`. El cronómetro empieza dentro del ejecutable cliente, por lo que excluye la compilación.

```sh
CRUD_TEST_MODE=benchmark cargo run --locked -p oxido-cli -- ejecutar prueba/crud_sqlite/verificar_persistencia.ox --confiar-dependencias
```

Esto es una medición de carga pequeña y exploratoria, no un objetivo ni una garantía de rendimiento. Los ticks de CPU son unidades de `/proc` y el RSS máximo es el mayor valor observado por el muestreador de 10 ms; los resultados dependen de la máquina y del estado del proceso. Para comparar versiones hay que fijar el hardware, aislar la máquina, aumentar y repetir cargas, y conservar los informes.

La API escucha por defecto en `127.0.0.1:38212`; `CRUD_ADDR` permite cambiarlo. El cliente admite `CRUD_TEST_ADDR` y `CRUD_TEST_STATE_PATH`. Los casos CRUD sin base de datos y las pruebas de interoperabilidad con `std` siguen separados en sus proyectos existentes bajo `prueba/`.

La línea base 100 % Rust y el procedimiento que usa **un mismo binario cliente** para comparar ambos servidores están en [`../crud_sqlite_rust/README.md`](../crud_sqlite_rust/README.md).
