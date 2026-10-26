# Benchmark release comparativo en GitHub Actions

## Ejecución

| Dato | Valor |
|---|---|
| Fecha | 2026-10-26 |
| Workflow run | [40318401833](https://github.com/programadorkevinsvega-sys/oxido-lengue/actions/runs/40318401833) |
| Commit probado | `046bce954ae561fa77e767b7a2592779a16e306b` |
| Artefacto | `oxido-rust-benchmark-046bce954ae561fa77e767b7a2592779a16e306b` |
| Host reportado por Actions | Linux, imagen `ubuntu26` |
| Toolchain | rustc/cargo 1.99.1 |
| Carga | HTTP/1, SQLite; concurrencia 50/100/200; cinco rondas con orden alternado; dos lotes por concurrencia y servidor en cada ronda (10 muestras por celda) |
| Perfil | `--release`; compilación y calentamiento fuera del intervalo medido |
| Fallos HTTP | Cero en todos los lotes |

El mismo cliente `benchmark-rust`, las mismas dependencias y configuración de SQLite ejecutaron ambos binarios de forma secuencial. Cada proceso se detenía y se comprobaba que el puerto quedara libre antes de iniciar el siguiente. Se midieron QPS y latencias p50/p95/p99. El recolector de recursos informa CPU en ticks de `/proc` y pico RSS muestreado cada 10 ms; estos valores son aproximados y específicos de Linux.

## Resultados resumidos

Medianas de las 10 muestras por carga; QPS también muestra el rango observado para hacer visible la variabilidad.

| Implementación | Concurrencia | QPS mediana (rango) | p50 mediana | p95 mediana | p99 mediana |
|---|---:|---:|---:|---:|---:|
| Óxido generado | 50 | 2437.83 (1386.12–4010.04) | 5.70 ms | 12.51 ms | 19.69 ms |
| Rust nativo | 50 | 2255.82 (1410.26–3115.39) | 5.11 ms | 12.47 ms | 21.62 ms |
| Óxido generado | 100 | 2691.82 (1249.29–3401.97) | 10.27 ms | 16.97 ms | 29.45 ms |
| Rust nativo | 100 | 2807.38 (1232.16–3392.20) | 10.31 ms | 17.82 ms | 25.48 ms |
| Óxido generado | 200 | 3308.42 (182.23–3637.87) | 22.95 ms | 32.12 ms | 43.84 ms |
| Rust nativo | 200 | 3369.91 (2434.16–3671.16) | 21.41 ms | 30.24 ms | 39.53 ms |

El pico RSS muestreado quedó entre 11,436–14,812 KiB para Óxido y 11,884–13,060 KiB para Rust nativo. Las muestras registraron 24–25 ticks de CPU por ejecución del cliente. Un QPS de Óxido a concurrencia 200 cayó a 182.23; ese outlier y los rangos amplios muestran variabilidad del runner. Las medianas cercanas no prueban equivalencia de rendimiento ni permiten concluir que una implementación sea generalmente más rápida.

## Alcance y límites

Esto demuestra que ambos binarios release completaron esta carga en el runner indicado sin errores HTTP. Es una medición exploratoria de esta API, dependencias, SQLite y entorno compartido de GitHub Actions; no mide el rendimiento del compilador, no aísla causalidad del lenguaje y no representa capacidad de producción. No se controlaron CPU/memoria de un host dedicado ni se repitió en hardware independiente.

El log íntegro está en el artefacto del workflow enlazado arriba. La salida del ejecutable identifica el benchmark como exploratorio; no se copia como evidencia de benchmark local.
