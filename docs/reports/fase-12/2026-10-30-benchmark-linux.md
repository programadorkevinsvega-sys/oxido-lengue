# Benchmark de análisis, generación y LSP (Linux, Windows y macOS)

**Fecha:** 2026-10-30 (medición local), 2026-10-31 (CI) y 2026-11-01 (corrección del método de RSS)

**Commit de referencia:** cambios de Fase 12.8 y corrección del método de RSS (2026-11-01)

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

- Un proceso hijo por escenario para que el pico de memoria no se contamine con los escenarios anteriores.
- Un hijo mide los tiempos y otro ejecuta el escenario una sola vez para medir el RSS; así el asignador no acumula memoria entre iteraciones.
- RSS máximo muestreado cada 2 ms por `sysinfo` desde el proceso padre. El hijo de memoria informa además su RSS base al arrancar, que se conserva si el escenario termina antes del primer muestreo. El valor es una cota inferior del pico real: un pico más corto que el intervalo puede no capturarse.
- Calentamiento y número de iteraciones variables según el coste del escenario. La columna `Iteraciones` corresponde a la medición de tiempos; la memoria siempre usa una ejecución.
- Entrada grande acotada al límite real del lexer: 4 MiB.
- La entrada grande válida contiene 8.222 funciones pequeñas, no una única función gigante, para representar un archivo grande sin convertir el benchmark en una prueba de una forma patológica.
- `--check` falla si se supera el P95 o el RSS objetivo, si vence el plazo de 10 s o si el escenario no hizo el trabajo esperado (por ejemplo, generar cero bytes de Rust o no devolver símbolos).

## Resultados

| Escenario | Entrada | Iteraciones | Mín (ms) | Mediana (ms) | P95 (ms) | Máx (ms) | RSS máx (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| `lex-typical` | 4.9 KiB | 20 | 0.05 | 0.05 | 0.05 | 0.05 | 2.8 |
| `parse-typical` | 4.9 KiB | 20 | 0.08 | 0.14 | 0.15 | 0.21 | 2.7 |
| `generate-typical` | 4.9 KiB | 10 | 23.62 | 23.88 | 24.21 | 24.21 | 6.1 |
| `lex-large` | 4.0 MiB | 10 | 44.06 | 44.71 | 45.71 | 45.71 | 39.0 |
| `parse-large` | 4.0 MiB | 10 | 68.26 | 68.87 | 79.53 | 79.53 | 91.2 |
| `generate-large` | 4.0 MiB | 2 | 3234.03 | 3317.36 | 3400.70 | 3400.70 | 721.6 |
| `lex-invalid` | 4.0 MiB | 3 | 189.19 | 189.74 | 190.01 | 190.01 | 7.0 |
| `parse-invalid` | 4.0 MiB | 3 | 192.87 | 193.34 | 193.74 | 193.74 | 7.1 |
| `lsp-open-typical` | 4.9 KiB | 10 | 0.95 | 1.05 | 1.21 | 1.21 | 3.2 |
| `lsp-hover-typical` | 4.9 KiB | 50 | 0.53 | 0.54 | 0.74 | 0.81 | 5.2 |
| `lsp-symbols-typical` | 4.9 KiB | 20 | 0.48 | 0.50 | 0.99 | 1.01 | 5.1 |
| `lsp-references-typical` | 4.9 KiB | 20 | 0.58 | 0.59 | 0.65 | 0.73 | 5.2 |
| `lsp-open-large` | 4.0 MiB | 1 | 393.39 | 393.39 | 393.39 | 393.39 | 114.5 |

Los escenarios `lsp-*` miden el coste dentro del proceso del servidor mediante `handle_json`; no incluyen la latencia de un cliente editor ni el transporte stdio real.

## Objetivos y estado

| Escenario | P95 observado (ms) | Límite P95 (ms) | RSS observado (MiB) | Límite RSS (MiB) | Estado |
|---|---:|---:|---:|---:|:---:|
| `lex-typical` | 0.05 | 50 | 2.8 | 64 | Aprobado |
| `parse-typical` | 0.15 | 50 | 2.7 | 64 | Aprobado |
| `generate-typical` | 24.21 | 500 | 6.1 | 128 | Aprobado |
| `lex-large` | 45.71 | 2000 | 39.0 | 128 | Aprobado |
| `parse-large` | 79.53 | 2000 | 91.2 | 256 | Aprobado |
| `generate-large` | 3400.70 | 10000 | 721.6 | 1536 | Aprobado |
| `lex-invalid` | 190.01 | 2000 | 7.0 | 128 | Aprobado |
| `parse-invalid` | 193.74 | 2000 | 7.1 | 128 | Aprobado |
| `lsp-open-typical` | 1.21 | 100 | 3.2 | 128 | Aprobado |
| `lsp-hover-typical` | 0.74 | 50 | 5.2 | 128 | Aprobado |
| `lsp-symbols-typical` | 0.99 | 50 | 5.1 | 128 | Aprobado |
| `lsp-references-typical` | 0.65 | 50 | 5.2 | 128 | Aprobado |
| `lsp-open-large` | 393.39 | 3000 | 114.5 | 512 | Aprobado |

Los objetivos se fijaron después de medir y dejan margen para la variabilidad del runner. No son objetivos de producción ni una promesa de rendimiento.

## CI multiplataforma

Run de CI [41277774631](https://github.com/programadorkevinsvega-sys/oxido-lengue/actions/runs/41277774631), job `benchmark-analysis`, con el método de RSS corregido. Los tres runners ejecutaron `cargo run --release --locked -p oxido-benchmark -- --check` y aprobaron todos los objetivos.

| Runner | Sistema | CPUs lógicas |
|---|---|---:|
| `ubuntu-latest` | Linux `x86_64` | 4 |
| `windows-latest` | Windows `x86_64` | 4 |
| `macos-14` | macOS `aarch64` | 3 |

P95 en ms / RSS máximo en MiB:

| Escenario | Ubuntu | Windows | macOS |
|---|---:|---:|---:|
| `lex-typical` | 0.08 / 3.0 | 0.05 / 7.0 | 0.05 / 3.2 |
| `parse-typical` | 0.12 / 3.1 | 0.14 / 7.0 | 0.08 / 3.2 |
| `generate-typical` | 24.07 / 6.0 | 46.66 / 9.3 | 91.40 / 5.3 |
| `lex-large` | 55.55 / 39.3 | 35.71 / 56.0 | 53.53 / 40.3 |
| `parse-large` | 83.76 / 91.5 | 185.22 / 95.2 | 81.27 / 93.3 |
| `generate-large` | 3790.03 / 722.9 | 3633.69 / 609.1 | 2987.30 / 708.0 |
| `lex-invalid` | 238.36 / 7.3 | 224.94 / 11.4 | 307.48 / 7.5 |
| `parse-invalid` | 238.65 / 7.3 | 227.46 / 12.1 | 315.19 / 7.6 |
| `lsp-open-typical` | 0.59 / 3.5 | 0.77 / 8.5 | 0.60 / 3.4 |
| `lsp-hover-typical` | 0.61 / 5.4 | 0.84 / 8.8 | 0.59 / 4.5 |
| `lsp-symbols-typical` | 0.55 / 5.5 | 0.79 / 8.1 | 0.53 / 4.5 |
| `lsp-references-typical` | 0.70 / 5.6 | 0.89 / 7.9 | 0.68 / 4.6 |
| `lsp-open-large` | 430.00 / 114.9 | 521.63 / 112.3 | 394.64 / 140.5 |

Los runners de GitHub tienen menos CPUs que la máquina local (12), así que sus tiempos no son comparables uno a uno con la tabla anterior. Los valores sí confirman que la generación de 4 MiB termina dentro del plazo y que el RSS se mantiene por debajo de los objetivos en los tres sistemas.

## Corrección del método de RSS (2026-11-01)

La primera medición de CI en `main` falló de forma intermitente en `benchmark-analysis (macos-14)`: el mismo commit produjo 46,9, 122,1 y 134,3 MiB para `lex-large` con el límite de 128 MiB. La causa no era una regresión del lexer:

- El muestreador vivía dentro del proceso medido y competía con el propio benchmark.
- La memoria se medía tras 3–10 iteraciones en el mismo proceso, así que el asignador podía retener páginas entre iteraciones y el pico dependía de cuántas se hubieran acumulado.
- Con 10 iteraciones, cinco ejecuciones en `macos-14` dieron 59, 71, 189, 294 y 300 MiB para el mismo escenario.

El método corregido separa los tiempos (hijo con varias iteraciones) del RSS (hijo con una sola ejecución, muestreado por el padre cada 2 ms). Con este método, seis ejecuciones en `macos-14` dieron 28,8–40,3 MiB para `lex-large` y 92,8–111,8 MiB para `parse-large`. Los objetivos de 128 y 256 MiB no necesitaron cambios; el problema era la medición, no el presupuesto.

## Límites encontrados

- La generación válida de 4 MiB tarda unos 3,4 s y alcanza 721,6 MiB de RSS; queda por debajo del plazo de 10 s y del objetivo de 1.536 MiB. Una medición anterior con el método de memoria acumulada dio 1.022 MiB; la diferencia refuerza que el RSS de una sola ejecución y el de varias iteraciones no son intercambiables.
- Una única función con unas 300.000 sentencias (también dentro del límite de 4 MiB) desborda la pila de `rustfmt` y devuelve `OX4001`; no hay pánico del compilador. El benchmark usa muchas funciones pequeñas para medir una carga representativa, pero el caso patológico sigue siendo un límite conocido: no toda entrada válida de 4 MiB se genera correctamente.
- Los escenarios LSP interactivos rondan 1 ms en esta máquina; el benchmark no mide el round-trip completo del editor.
- La medición local es Linux con 12 CPUs; la de CI usa runners de 4, 4 y 3 CPUs. Son muestras de ese hardware y ese toolchain, no garantías para otros equipos.
- Los runners de CI no son entornos de producción y pueden variar entre ejecuciones; los objetivos dejan margen, pero no convierten estos números en SLA.
- El RSS es una cota inferior del pico real: el muestreo cada 2 ms puede no capturar un pico más corto. No se usa `getrusage`/`rusage` del sistema operativo.
- No se ha medido el coste de `cargo build` ni el tiempo de compilación Rust; este informe cubre lexer, parser, generación y LSP.

## Reproducción

```bash
cargo run --release --locked -p oxido-benchmark
cargo run --release --locked -p oxido-benchmark -- --check
```

El primer comando imprime el informe; el segundo añade la evaluación de objetivos y devuelve un código de salida distinto de cero si alguna comprobación falla.
