# Diagnóstico, trazabilidad y rendimiento de Óxido

## Alcance

Este documento cubre la observabilidad local del compilador y sus herramientas: CLI, backend, extensión y LSP. No introduce telemetría remota ni servicios de monitoreo externos.

La telemetría remota está desactivada por defecto. Cualquier envío externo requerirá una decisión explícita, opción de configuración visible y documentación sobre los datos transmitidos. La funcionalidad normal del compilador no dependerá de una cuenta ni de una conexión externa de telemetría.

## Diagnósticos de compilación

- Cada diagnóstico tiene código estable, severidad, mensaje claro, archivo y rango de origen `.ox`.
- Conserva el mapa entre posiciones `.ox` y Rust generado desde la primera fase de compilación funcional.
- Lee los diagnósticos de Cargo mediante `--message-format=json`; no analices texto de terminal para reconstruir ubicaciones.
- Conserva el error original de Cargo/`rustc` junto con la explicación de Óxido. No inventes causas ni ocultes errores.
- Los mensajes no deben exponer secretos, datos personales ni rutas absolutas en informes compartidos.

## CLI y compilador

Los mensajes de progreso y error deben ser claros y deterministas. La salida legible para personas puede complementarse con un formato estructurado para herramientas automatizadas. Los códigos de salida deben reflejar el resultado real del análisis o compilación.

La instrumentación de diagnóstico debe ser local y de bajo costo. Si se ofrece un modo detallado o de tiempos, debe activarse explícitamente y no cambiar la semántica del programa.

## LSP y editor

- Usa la misma gramática, clasificación de tokens y diagnósticos que el compilador.
- Expone semantic tokens desde el análisis de Óxido para que el resaltado coincida con el compilador.
- Usa `rust-analyzer` sobre el Rust generado para tipos y autocompletado cuando el mapa de posiciones permita una correspondencia fiable.
- Maneja solicitudes incompletas, cancelación, documentos grandes y ráfagas sin bloquear el editor ni agotar recursos.
- No compiles ni ejecutes análisis que ejecuten código en workspaces no confiables; respeta Workspace Trust.

## Rendimiento y señales locales

Mide duración y consumo de memoria de lectura, lexing, parsing, generación, compilación y solicitudes LSP. Mantén benchmarks reproducibles y, cuando ayude al diagnóstico, ofrece un modo local de tiempos o salida estructurada con métricas por etapa. Mide tamaño de entrada/salida, cantidad y severidad de diagnósticos y cancelaciones del LSP; no incluyas el texto fuente en esas métricas. Usa trazas locales solo cuando aporten diagnóstico y limita su volumen.

Prometheus no forma parte de la instalación local por defecto. Evalúalo si Óxido ofrece un servicio compartido y de larga duración, como un playground o workers de compilación; en ese caso, instrumenta disponibilidad, solicitudes, errores, latencia, colas y recursos, con límites de cardinalidad y sin código fuente ni datos sensibles.

Los errores del sistema de diagnóstico no deben ocultar el error original ni impedir que la CLI o el LSP respondan cuando exista una degradación segura.

## Privacidad

No envíes telemetría por defecto. No registres el contenido completo del código fuente salvo que una operación local lo requiera y el usuario lo solicite. Normaliza rutas en informes exportables y excluye credenciales, variables de entorno sensibles y datos privados.

## Verificación

Comprueba diagnósticos con errores reales de parser y `rustc`, rangos con fixtures de regresión, solicitudes LSP reales y benchmarks repetibles. Informa qué comprobaciones se ejecutaron y no declares validada una integración que no se haya probado.
