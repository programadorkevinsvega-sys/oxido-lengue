# Educación y material didáctico

Esta página reúne la ruta de aprendizaje, los ejemplos verificados y los ejercicios propuestos. El material es inicial: no se ha validado con estudiantes ni con docentes, y no se afirma que mejore resultados de aprendizaje. Los ejemplos que se presentan como ejecutables están cubiertos por pruebas del repositorio.

## Estado

- **Verificado:** los ejemplos de la tabla siguiente se ejecutan con la CLI actual y forman parte de las pruebas de extremo a extremo.
- **Propuesto:** los ejercicios, la guía para docentes y el plan de pilotos son propuestas. No se han usado en un aula.
- **Fuera de alcance:** una plataforma de cursos, cuentas de usuario, corrección automática en red y telemetría educativa.

## Ruta de aprendizaje

1. **Primer programa.** Sintaxis indentada, función `principal`/`main`, `imprimir!`/`println!` y ejecución con la CLI. Ejemplo: `examples/hola_mundo.ox`.
2. **Estructuras y tipos.** `estructura`, `enumeracion`, `dejar`, mutabilidad y colecciones. Ejemplo: `examples/fase_7_estructuras.ox`.
3. **Interoperabilidad con Rust.** Módulos, métodos, tuplas, referencias, `Result`, `Option`, `?` y `VecDeque`. Ejemplo: `examples/fase_7_interoperabilidad.ox`.
4. **Estructura indentada.** `implementar`, `rasgo`, módulos anidados y `coincidir`. Ejemplo: `examples/fase_8_estructura_indentada.ox`.
5. **Lectura del Rust generado.** Exportar, comparar con `rustfmt` y relacionar los diagnósticos con el código fuente. Ejemplo: `examples/hola_mundo.ox`.

Consulta la [especificación del lenguaje](language-spec.md), los [modos de sintaxis](syntax-modes.md), los [alias](aliases.md) y la [matriz de compatibilidad](compatibility.md) antes de proponer sintaxis nueva.

## Ejemplos verificados

Desde la raíz del repositorio:

| Ejemplo | Concepto | Comando | Salida esperada |
|---|---|---|---|
| `examples/hola_mundo.ox` | Programa mínimo | `cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox` | `Hola, mundo.` |
| `examples/fase_7_estructuras.ox` | Estructuras, enumeraciones y colecciones | `cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_estructuras.ox` | `5:1` |
| `examples/fase_7_interoperabilidad.ox` | Métodos, referencias y `?` | `cargo run --locked -p oxido-cli -- ejecutar examples/fase_7_interoperabilidad.ox` | `5:1:3:5` |
| `examples/fase_8_estructura_indentada.ox` | Rasgos y `coincidir` | `cargo run --locked -p oxido-cli -- ejecutar examples/fase_8_estructura_indentada.ox` | `valor 4: cuatro` |

El ejemplo de Fase 8 puede emitir además un aviso `dead_code` de Rust; no impide la ejecución.

Para exportar y leer el Rust generado:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
```

`exportar` crea `target/hola.oxido-map.json` con el mapa de origen. El Rust final se formatea con `rustfmt` 1.99.1.

## Ejercicios propuestos

Los ejercicios indican criterios observables. No hay corrección automática ni soluciones publicadas; el docente puede comprobar la salida y el diagnóstico.

### 1. Cambiar el saludo

Modifica una copia de `hola_mundo.ox` para que imprima tu nombre.

- **Criterio:** `oxido ejecutar` termina con código 0 y muestra el texto nuevo.
- **Concepto:** función de entrada, cadena literal, macro de impresión.
- **Límite:** no se evalúa estilo ni cobertura.

### 2. Añadir un campo a una estructura

Parte de `fase_7_estructuras.ox` y añade un campo a la estructura existente.

- **Criterio:** el programa compila, se ejecuta y la salida conserva el formato esperado o el que documente el estudiante.
- **Concepto:** tipos, inicialización, acceso a campos.
- **Límite:** la matriz de compatibilidad delimita los tipos admitidos.

### 3. Provocar y leer un diagnóstico

Introduce un error de tipo en una copia de un ejemplo.

- **Criterio:** `oxido compilar` falla, muestra un código `OX*` o un diagnóstico de Rust y no hay pánico.
- **Concepto:** lectura de errores, diferencia entre frontend y `rustc`.
- **Límite:** algunos errores se detectan en Rust, no en el parser de Óxido.

### 4. Exportar y comparar

Exporta un ejemplo y localiza una construcción de Óxido en el Rust generado.

- **Criterio:** el estudiante señala la línea del `.rs` y explica qué parte proviene del frontend y qué parte conserva Rust.
- **Concepto:** pipeline, mapa de origen, `rustfmt`.
- **Límite:** el texto `.ox` no es idéntico al `.rs`; la comparación es estructural.

### 5. Ejecutar las pruebas de un archivo

Convierte un ejemplo en un archivo con una función `#[test]` y ejecútalo con `oxido probar`.

- **Criterio:** `oxido probar` informa al menos una prueba ejecutada y el resultado esperado.
- **Concepto:** integración con Cargo, pruebas.
- **Límite:** un archivo sin `#[test]` puede informar cero pruebas sin ser un error.

## Guía para docentes

- Usa la salida real de la CLI como referencia, no capturas de una versión distinta.
- Distingue siempre **implementado y verificado**, **especificado pero pendiente** y **propuesta futura**.
- No presentes la matriz de compatibilidad como si cubriera todo Rust.
- No pidas a los estudiantes ejecutar dependencias no confiables. La compilación con dependencias ejecuta `build.rs` y macros.
- Si el curso necesita Windows o macOS, comprueba la CLI en esa plataforma; el sandbox del playground solo existe en Linux.
- Recoge solo los datos necesarios y con consentimiento. El proyecto no envía telemetría.

## Plan de pilotos educativos

Propuesta, sin ejecución ni resultados:

1. **Objetivo:** evaluar si el material permite completar los ejercicios 1–3 sin ayuda del docente y si los conceptos de Rust se comprenden mejor que con el material de control.
2. **Diseño:** grupo piloto y grupo de comparación, mismos ejercicios y mismo tiempo, con pre y post prueba. La asignación debe ser voluntaria y reversible.
3. **Evidencia:** tasa de finalización, diagnósticos encontrados, tiempo hasta la primera ejecución correcta, encuesta de claridad y errores de concepto.
4. **Privacidad:** datos agregados, sin nombres, rutas ni identificadores personales. Consentimiento informado y posibilidad de retirarse.
5. **Límites:** una muestra pequeña no demuestra eficacia general. No se publicarán conclusiones sin el análisis y la revisión correspondientes.
6. **Criterio de avance:** solo se declarará un piloto completado cuando existan protocolo, datos, análisis y limitaciones documentados.

## Material bilingüe

La documentación canónica está en español. El sitio publica solo la locale `es`. Una guía en inglés se añadirá cuando exista una versión mantenida y revisada; no se traducirá automáticamente ni se presentará como equivalente sin revisión.

## Añadir un ejemplo

1. Crea el archivo `.ox` en `examples/`.
2. Añade una prueba que lo ejecute o lo compile.
3. Documenta el concepto, el comando y la salida esperada.
4. Marca como propuesto lo que aún no esté cubierto por pruebas.
5. Verifica `cargo test --workspace --locked` y el build del sitio.
