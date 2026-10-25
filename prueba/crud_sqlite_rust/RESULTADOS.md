# Resultados de comparación local

Fecha: 2026-10-25. Se hicieron dos ejecuciones de comparación; en cada una, el mismo ejecutable `benchmark-rust` realizó dos lotes de cada tamaño (50, 100 y 200) contra ambos servidores. Se reutilizó `/tmp/crud-sqlite-comparacion.sqlite`; antes de cada servidor se activó `CRUD_RESET_DB=1`. Hubo 0 respuestas erróneas en todos los lotes válidos.

## Medición histórica en perfil debug

La primera comparación se hizo con `cargo build`/`cargo run`, sin `--release`. Se conserva como antecedente; **no representa rendimiento optimizado**.

## Entorno de esa medición

- Linux x86_64, kernel `7.0.0-38-generic`, Ubuntu 24.04.
- Intel Core i5-12450H, 8 núcleos / 12 CPUs lógicas.
- `lscpu` reportó el escalado de frecuencia en 72 % durante la observación.
- Axum, Serde, Tokio y SQLx usaron las mismas versiones, features y checksums de crates.io en ambos lockfiles.
- La compilación de ambos servidores y del cliente se completó antes de medir.

## Medianas de cuatro lotes por tamaño

La mediana resume cuatro mediciones; el rango de QPS deja visible la variación entre lotes.

| Solicitudes simultáneas | Servidor | QPS mediano (rango) | p50 mediana | p95 mediana | p99 mediana |
|---:|---|---:|---:|---:|---:|
| 50 | Óxido generado | 660 (213–1.378) | 16,16 ms | 49,18 ms | 85,93 ms |
| 50 | Rust nativo | 421 (376–609) | 12,46 ms | 69,06 ms | 119,46 ms |
| 100 | Óxido generado | 872 (428–1.210) | 34,84 ms | 76,33 ms | 100,45 ms |
| 100 | Rust nativo | 1.203 (931–1.548) | 24,10 ms | 43,61 ms | 56,93 ms |
| 200 | Óxido generado | 1.263 (315–1.795) | 72,16 ms | 130,94 ms | 170,16 ms |
| 200 | Rust nativo | 1.501 (1.093–2.094) | 44,96 ms | 81,25 ms | 102,58 ms |

## Recursos del proceso servidor

| Servidor | CPU ticks por ejecución (dos lotes por tamaño) | RSS máximo muestreado |
|---|---:|---:|
| Óxido generado | 37 y 37 | 16.736 y 16.528 KiB |
| Rust nativo | 36 y 35 | 17.236 y 16.596 KiB |

Los ticks de `/proc` corresponden a 100 ticks por segundo en esta máquina; suman CPU de proceso, no tiempo de pared. El RSS es el máximo observado por muestreo cada 10 ms, no un máximo garantizado.

## Lectura

Con 100 y 200 solicitudes, el Rust nativo tuvo mayor QPS mediano y menor latencia mediana en esta muestra. Con 50 solicitudes el QPS mediano favoreció a Óxido, mientras que las colas p95/p99 favorecieron al Rust nativo. Los rangos amplios —por ejemplo, 315 a 1.795 QPS para Óxido con 200 solicitudes— muestran que la variación ambiental es grande frente a las diferencias observadas.

Esta muestra **no permite asignar un porcentaje estable de pérdida al lenguaje**.

## Comparación release secuencial

Fecha: 2026-10-25. Ambos servidores, el cliente y el Rust generado por Óxido se compilaron con perfil `release`. Se hicieron cinco rondas alternando el orden de los servidores, diez lotes por tamaño y servidor. Cada lote tuvo cero respuestas erróneas. Antes de cada servidor se vació la misma base SQLite; el supervisor esperó a que el proceso terminara y verificó que el puerto 38212 quedara libre antes de iniciar el siguiente.

El supervisor calentó ambos procesos con diez consultas `GET /productos`. El servidor generado por Óxido se compiló antes de medir y después se ejecutó directamente desde `target/release/oxido_generated`; Rust nativo se ejecutó desde `target/release/crud-api-rust`. Ambos recibieron el mismo ejecutable `benchmark-rust` desde `target/release`.

| Solicitudes simultáneas | Servidor | QPS mediano (rango de 10 lotes) | p50 mediana | p95 mediana | p99 mediana |
|---:|---|---:|---:|---:|---:|
| 50 | Óxido generado | 541,75 (273,45–1.418,37) | 9,94 ms | 45,25 ms | 93,25 ms |
| 50 | Rust nativo | 542,70 (272,79–1.785,46) | 9,64 ms | 45,97 ms | 91,34 ms |
| 100 | Óxido generado | 932,84 (300,12–1.242,42) | 15,70 ms | 34,61 ms | 80,81 ms |
| 100 | Rust nativo | 762,91 (755,89–1.769,23) | 15,09 ms | 34,42 ms | 105,55 ms |
| 200 | Óxido generado | 1.515,52 (858,88–1.877,67) | 31,55 ms | 57,45 ms | 85,88 ms |
| 200 | Rust nativo | 1.509,72 (856,97–1.888,65) | 28,91 ms | 55,06 ms | 80,20 ms |

CPU ticks medianos por ejecución completa: 15 para ambos. RSS máximo mediano muestreado: 14.556 KiB para Óxido y 14.428 KiB para Rust. Son cinco observaciones por servidor, muestreadas cada 10 ms.

### Controles de equivalencia

- `oxido.toml` y `Cargo.toml` declaran las mismas versiones y features para Axum, Serde, Tokio y SQLx; sus `Cargo.lock` tienen el mismo grafo de crates, versiones y checksums. Solo difiere el nombre del paquete raíz.
- Los dos servidores usan SQLite con el mismo archivo por ejecución, WAL, ocho conexiones, timeout de 10 s, esquema, consultas SQL, rutas, validaciones, transacciones y respuestas HTTP. En Óxido los campos se llaman en español; en Rust nativo usan `serde(rename)` para producir el mismo JSON.
- Se exportó el programa `.ox` a Rust y se revisaron las diferencias con `src/main.rs`: son principalmente nombres y forma sintáctica. Los handlers usan las mismas operaciones y consultas; por ejemplo, obtener un registro se expresa como `coincidir` en Óxido y como `Option::map` en Rust. La exportación temporal usada para la inspección se retiró.
- `benchmark_secuencial.py` serializa el ciclo y se niega a iniciar si el puerto ya está ocupado. La primera compilación del Rust generado no cuenta como tiempo medido.

La máquina no estaba aislada: durante la prueba también estaban abiertos Brave con ocho pestañas, VS Code, Codex CLI y un editor de notas, y seguían activos contenedores de Docker para Nginx, Prometheus, NATS, PostgreSQL, Redis, Grafana, Loki, Tempo, OpenTelemetry y exporters. Estos procesos pudieron competir por CPU, memoria y E/S; `docker ps` confirma que estaban activos, pero no cuánto recurso consumió cada uno durante cada lote.

`lscpu` indicó escalado de frecuencia al 46 % y los rangos de QPS son amplios. Las medianas observadas son cercanas a 50 y 200 solicitudes; a 100 la mediana de QPS varía a favor de Óxido, pero las mediciones individuales oscilan mucho. **No hay evidencia de una penalización estable por traducir las palabras**, y esta ejecución tampoco permite comparar el rendimiento intrínseco de los dos binarios con precisión. Para eso se necesita repetir en un entorno más quieto, registrar carga/CPU durante cada lote y perfilar CPU si aparece una diferencia reproducible.
