# Benchmark de análisis, generación y LSP (Linux, Windows y macOS)

**Fecha:** 2026-10-30 (medición local) y 2026-10-31 (CI)

**Commit de referencia:** cambios de Fase 12.8 (sin fusionar al crear este informe)

**Herramienta:** `tools/benchmark` (`oxido-benchmark`)

## Entorno

| Dato | Valor |
|---|---|
| Sistema | Linux `x86_64` |
| CPUs lógicas | 12 |
| Rust | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |
| Perfil | `release` |
| Presupuesto de análisis | 10 s |

## Método

- Un proceso hijo por escenario para que el pico de RSS no se contamine con escenarios anteriores.
- Calentamiento y número de iteraciones variables según el coste del escenario.
- RSS máximo muestreado cada 20 ms por `sysinfo`; se conserva el mayor valor entre el muestreo y la lectura final.
- Entrada grande acotada al límite real del lexer: 4 MiB.
- La entrada grande válida contiene 8.222 funciones pequeñas, no una única función gigante, para representar un archivo grande sin convertir el benchmark en una prueba de una forma patológica.
- `--check` falla si se supera el P95 o el RSS objetivo, si vence el plazo de 10 s o si el escenario no hizo el trabajo esperado (por ejemplo, generar cero bytes de Rust o no devolver símbolos).

## Resultados

| Escenario | Entrada | Iteraciones | Mín (ms) | Mediana (ms) | P95 (ms) | Máx (ms) | RSS máx (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| `lex-typical` | 4.9 KiB | 20 | 0.09 | 0.09 | 0.10 | 0.10 | 2.9 |
| `parse-typical` | 4.9 KiB | 20 | 0.14 | 0.14 | 0.16 | 0.16 | 3.3 |
| `generate-typical` | 4.9 KiB | 10 | 23.60 | 23.88 | 25.33 | 25.33 | 8.3 |
| `lex-large` | 4.0 MiB | 3 | 40.99 | 41.15 | 41.23 | 41.23 | 40.2 |
| `parse-large` | 4.0 MiB | 3 | 66.03 | 66.38 | 72.63 | 72.63 | 107.4 |
| `generate-large` | 4.0 MiB | 2 | 3281.22 | 3358.88 | 3436.55 | 3436.55 | 1022.0 |
| `lex-invalid` | 4.0 MiB | 3 | 212.37 | 212.43 | 213.05 | 213.05 | 7.3 |
| `parse-invalid` | 4.0 MiB | 3 | 208.70 | 213.44 | 215.52 | 215.52 | 7.3 |
| `lsp-open-typical` | 4.9 KiB | 10 | 0.53 | 0.97 | 1.05 | 1.05 | 5.8 |
| `lsp-hover-typical` | 4.9 KiB | 50 | 0.54 | 0.57 | 1.05 | 1.18 | 8.4 |
| `lsp-symbols-typical` | 4.9 KiB | 20 | 0.48 | 0.51 | 0.88 | 0.99 | 6.6 |
| `lsp-references-typical` | 4.9 KiB | 20 | 0.58 | 0.60 | 0.63 | 0.66 | 6.6 |
| `lsp-open-large` | 4.0 MiB | 1 | 415.84 | 415.84 | 415.84 | 415.84 | 114.3 |

Los escenarios `lsp-*` miden el coste dentro del proceso del servidor mediante `handle_json`; no incluyen la latencia de un cliente editor ni el transporte stdio real.

## Objetivos y estado

| Escenario | P95 observado (ms) | Límite P95 (ms) | RSS observado (MiB) | Límite RSS (MiB) | Estado |
|---|---:|---:|---:|---:|:---:|
| `lex-typical` | 0.10 | 50 | 2.9 | 64 | Aprobado |
| `parse-typical` | 0.16 | 50 | 3.3 | 64 | Aprobado |
| `generate-typical` | 25.33 | 500 | 8.3 | 128 | Aprobado |
| `lex-large` | 41.23 | 2000 | 40.2 | 128 | Aprobado |
| `parse-large` | 72.63 | 2000 | 107.4 | 256 | Aprobado |
| `generate-large` | 3436.55 | 10000 | 1022.0 | 1536 | Aprobado |
| `lex-invalid` | 213.05 | 2000 | 7.3 | 128 | Aprobado |
| `parse-invalid` | 215.52 | 2000 | 7.3 | 128 | Aprobado |
| `lsp-open-typical` | 1.05 | 100 | 5.8 | 128 | Aprobado |
| `lsp-hover-typical` | 1.05 | 50 | 8.4 | 128 | Aprobado |
| `lsp-symbols-typical` | 0.88 | 50 | 6.6 | 128 | Aprobado |
| `lsp-references-typical` | 0.63 | 50 | 6.6 | 128 | Aprobado |
| `lsp-open-large` | 415.84 | 3000 | 114.3 | 512 | Aprobado |

Los objetivos se fijaron después de medir y dejan margen para la variabilidad del runner. No son objetivos de producción ni una promesa de rendimiento.

## CI multiplataforma

Run de CI [41167513550](https://github.com/programadorkevinsvega-sys/oxido-lengue/actions/runs/41167513550), job `benchmark-analysis`. Los tres runners ejecutaron `cargo run --release --locked -p oxido-benchmark -- --check` y aprobaron todos los objetivos.

| Runner | Sistema | CPUs lógicas |
|---|---|---:|
| `ubuntu-latest` | Linux `x86_64` | 4 |
| `windows-latest` | Windows `x86_64` | 4 |
| `macos-14` | macOS `aarch64` | 3 |

P95 en ms / RSS máximo en MiB:

| Escenario | Ubuntu | Windows | macOS |
|---|---:|---:|---:|
| `lex-typical` | 0.13 / 3.1 | 0.13 / 7.4 | 0.06 / 3.7 |
| `parse-typical` | 0.15 / 3.2 | 0.31 / 7.4 | 0.09 / 3.8 |
| `generate-typical` | 25.83 / 8.4 | 72.18 / 11.9 | 179.54 / 9.4 |
| `lex-large` | 71.79 / 35.8 | 71.22 / 57.3 | 60.40 / 59.0 |
| `parse-large` | 115.16 / 92.6 | 160.92 / 95.7 | 106.82 / 117.5 |
| `generate-large` | 4445.12 / 1022.3 | 4625.40 / 826.0 | 3624.53 / 1005.5 |
| `lex-invalid` | 302.67 / 7.5 | 333.26 / 11.5 | 315.70 / 8.6 |
| `parse-invalid` | 298.58 / 7.5 | 292.82 / 11.6 | 324.47 / 8.4 |
| `lsp-open-typical` | 0.86 / 6.1 | 1.21 / 9.4 | 1.06 / 6.0 |
| `lsp-hover-typical` | 0.90 / 8.4 | 1.30 / 11.1 | 0.68 / 7.8 |
| `lsp-symbols-typical` | 0.82 / 6.7 | 1.10 / 9.5 | 1.53 / 6.1 |
| `lsp-references-typical` | 0.98 / 6.9 | 2.24 / 9.7 | 1.31 / 6.4 |
| `lsp-open-large` | 620.50 / 114.7 | 724.78 / 111.7 | 414.82 / 132.7 |

Los runners de GitHub tienen menos CPUs que la máquina local (12), así que sus tiempos no son comparables uno a uno con la tabla anterior. Los valores sí confirman que la generación de 4 MiB termina dentro del plazo y que el RSS se mantiene por debajo de los objetivos en los tres sistemas.

## Límites encontrados

- La generación válida de 4 MiB tarda unos 3,4 s y alcanza 1.022 MiB de RSS; queda por debajo del plazo de 10 s y del objetivo de 1.536 MiB, pero el margen de memoria no es amplio. Una segunda ejecución local dio 4,24 s y 1.017 MiB; la variación refuerza que los objetivos son cotas de CI, no umbrales de producción.
- Una única función con unas 300.000 sentencias (también dentro del límite de 4 MiB) desborda la pila de `rustfmt` y devuelve `OX4001`; no hay pánico del compilador. El benchmark usa muchas funciones pequeñas para medir una carga representativa, pero el caso patológico sigue siendo un límite conocido: no toda entrada válida de 4 MiB se genera correctamente.
- Los escenarios LSP interactivos rondan 1 ms en esta máquina; el benchmark no mide el round-trip completo del editor.
- La medición local es Linux con 12 CPUs; la de CI usa runners de 4, 4 y 3 CPUs. Son muestras de ese hardware y ese toolchain, no garantías para otros equipos.
- Los runners de CI no son entornos de producción y pueden variar entre ejecuciones; los objetivos dejan margen, pero no convierten estos números en SLA.
- No se ha medido el coste de `cargo build` ni el tiempo de compilación Rust; este informe cubre lexer, parser, generación y LSP.

## Reproducción

```bash
cargo run --release --locked -p oxido-benchmark
cargo run --release --locked -p oxido-benchmark -- --check
```

El primer comando imprime el informe; el segundo añade la evaluación de objetivos y devuelve un código de salida distinto de cero si alguna comprobación falla.
