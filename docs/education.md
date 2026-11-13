# Educación y material didáctico

Esta página reúne la ruta de aprendizaje, los ejemplos verificados y los ejercicios propuestos. El material es inicial: no se ha validado con estudiantes ni con docentes, y no se afirma que mejore resultados de aprendizaje. Los ejemplos que se presentan como ejecutables están cubiertos por pruebas del repositorio. Para una primera clase práctica, empieza por el [tutorial progresivo](tutorial.md), con tres lecciones ejecutables.

## Estado

- **Verificado:** los ejemplos de la tabla siguiente se ejecutan con la CLI actual y forman parte de las pruebas de extremo a extremo.
- **Propuesto/no ejecutado:** Libro #4–#9 del [libro de ejercicios](ejercicios.md) y cualquier piloto en aula. La guía docente y el [protocolo de pilotos](pilotos-educativos.md) ya están escritos, pero el material no se ha probado con estudiantes ni docentes.
- **Fuera de alcance:** una plataforma de cursos, cuentas de usuario, corrección automática en red y telemetría educativa.

## Ruta de aprendizaje

1. **Primer programa.** Sintaxis indentada, función `principal`/`main`, `imprimir!`/`println!` y ejecución con la CLI. Ejemplo: `examples/hola_mundo.ox`.
2. **Estructuras y tipos.** `estructura`, `enumeracion`, `dejar`, mutabilidad y colecciones. Ejemplo: `examples/estructuras.ox`.
3. **Interoperabilidad con Rust.** Módulos, métodos, tuplas, referencias, `Result`, `Option`, `?` y `VecDeque`. Ejemplo: `examples/interoperabilidad.ox`.
4. **Estructura indentada.** `implementar`, `rasgo`, módulos anidados y `coincidir`. Ejemplo: `examples/estructura_indentada.ox`.
5. **Lectura del Rust generado.** Exportar, comparar con `rustfmt` y relacionar los diagnósticos con el código fuente. Ejemplo: `examples/hola_mundo.ox`.

Consulta la [especificación del lenguaje](language-spec.md), los [modos de sintaxis](syntax-modes.md), los [alias](aliases.md) y la [matriz de compatibilidad](compatibility.md) antes de proponer sintaxis nueva.

## Ejemplos verificados

Desde la raíz del repositorio:

| Ejemplo | Concepto | Comando | Salida esperada |
|---|---|---|---|
| `examples/hola_mundo.ox` | Programa mínimo | `cargo run --locked -p oxido-cli -- ejecutar examples/hola_mundo.ox` | `Hola, mundo.` |
| `examples/estructuras.ox` | Estructuras, enumeraciones y colecciones | `cargo run --locked -p oxido-cli -- ejecutar examples/estructuras.ox` | `5:1` |
| `examples/interoperabilidad.ox` | Métodos, referencias y `?` | `cargo run --locked -p oxido-cli -- ejecutar examples/interoperabilidad.ox` | `5:1:3:5` |
| `examples/estructura_indentada.ox` | Rasgos y `coincidir` | `cargo run --locked -p oxido-cli -- ejecutar examples/estructura_indentada.ox` | `valor 4: cuatro` |

El ejemplo de estructura indentada puede emitir además un aviso `dead_code` de Rust; no impide la ejecución.

Para exportar y leer el Rust generado:

```sh
cargo run --locked -p oxido-cli -- exportar examples/hola_mundo.ox --salida target/hola.rs
```

`exportar` crea `target/hola.oxido-map.json` con el mapa de origen. El Rust final se formatea con `rustfmt` 1.99.1.

## Libro de ejercicios

La lista canónica es el [libro de ejercicios](ejercicios.md); esta página no mantiene una segunda numeración ni criterios duplicados. Los tres primeros ejercicios tienen inicio, solución de referencia y prueba automática. Los demás son propuestas con criterios observables. No hay corrección automática en red ni validación en aula.

| Identificador del libro | Tema | Estado técnico |
|---|---|---|
| Libro #1 | Promedio de un array | Solución ejecutada en CI |
| Libro #2 | Inventario con estructuras | Solución ejecutada en CI |
| Libro #3 | Contar palabras largas | Solución ejecutada en CI |
| Libro #4 | FizzBuzz acotado | Propuesto |
| Libro #5 | Buscar el máximo | Propuesto |
| Libro #6 | Contar vocales | Propuesto |
| Libro #7 | `Result` y propagación | Propuesto |
| Libro #8 | Módulos y visibilidad | Propuesto |
| Libro #9 | Leer el Rust generado | Propuesto |

## Guía para docentes

- Usa la salida real de la CLI como referencia, no capturas de una versión distinta.
- Distingue siempre **implementado y verificado**, **especificado pero pendiente** y **propuesta futura**.
- No presentes la matriz de compatibilidad como si cubriera todo Rust.
- No pidas a los estudiantes ejecutar dependencias no confiables. La compilación con dependencias ejecuta `build.rs` y macros.
- Si el curso necesita Windows o macOS, comprueba la CLI en esa plataforma; el sandbox del playground solo existe en Linux.
- Recoge solo los datos necesarios y con consentimiento. El proyecto no envía telemetría.

## Plan de pilotos educativos

El protocolo completo —diseño, instrumentos, consentimiento, privacidad, análisis y plantilla de informe— está en [Pilotos educativos](pilotos-educativos.md). **No se ha ejecutado ningún piloto.** Resumen:

1. **Objetivo:** evaluar si el material permite completar Libro #1–#3 sin ayuda del docente y si los conceptos de Rust se comprenden mejor que con el material de control.
2. **Diseño:** grupo piloto y grupo de comparación, mismos ejercicios y mismo tiempo, con pre y post prueba. La asignación debe ser voluntaria y reversible.
3. **Evidencia:** tasa de finalización, diagnósticos encontrados, tiempo hasta la primera ejecución correcta, encuesta de claridad y errores de concepto.
4. **Privacidad:** datos agregados, sin nombres, rutas ni identificadores personales. Consentimiento informado y posibilidad de retirarse.
5. **Límites:** una muestra pequeña no demuestra eficacia general. No se publicarán conclusiones sin el análisis y la revisión correspondientes.
6. **Criterio de avance:** solo se declarará un piloto completado cuando existan protocolo, datos, análisis y limitaciones documentados.

## Material bilingüe

La documentación canónica está en español. Se mantiene una [guía de inicio en inglés](en/quickstart.md) con instalación, primer programa, estructuras y enlaces a la referencia; el resto del sitio sigue en español y no se traduce automáticamente. La guía en inglés declara su alcance y no se presenta como equivalente completa. El libro de ejercicios y el protocolo de pilotos todavía no tienen versión inglesa mantenida.

## Añadir un ejemplo

1. Crea el archivo `.ox` en `examples/`.
2. Añade una prueba que lo ejecute o lo compile.
3. Documenta el concepto, el comando y la salida esperada.
4. Marca como propuesto lo que aún no esté cubierto por pruebas.
5. Verifica `cargo test --workspace --locked` y el build del sitio.
