# ADR-0002: generación trazable y adaptador Cargo

- **Estado:** Aceptada
- **Fecha:** 2026-10-25
- **Fase:** 5, primer compilador funcional

## Contexto

La Fase 4 produce un AST estructural y fragmentos Rust validados, pero todavía no genera `.rs`. La Fase 5 necesita preservar la semántica Rust, relacionar diagnósticos con el archivo `.ox` y ejecutar Cargo sin convertir entradas del usuario en comandos de shell.

## Decisión

- El backend convierte el AST con `proc_macro2`/`quote`, valida el archivo con Syn y usa `prettyplease` como formateador intermedio.
- El mapa lateral registra spans de AST y fragmentos de origen contra ubicaciones del `.rs` formateado. Se versiona para que CLI, Cargo y el editor compartan el contrato.
- El adaptador Cargo vive en el límite de infraestructura `oxido-cargo`; invoca `Command` con argumentos separados, directorio de trabajo explícito y mensajes JSON. El dominio del lenguaje no depende del proceso Cargo.
- Exportar escribe Rust y mapa inspeccionables. Compilar y ejecutar son acciones explícitas; los errores de Cargo que no se puedan mapear se conservan como errores externos.
- Importar acepta únicamente el subconjunto que pueda reconstruir sin descartar semántica; todo lo demás se rechaza con diagnóstico.

## Consecuencias

- La primera rebanada end-to-end se limita al AST ya parseable y usa un programa «Hola, mundo» como prueba de aceptación.
- Cargo/rustc recibe código generado de un proyecto mínimo controlado por Óxido; no se ejecuta automáticamente un `build.rs` de un workspace ajeno.
- Rust generado sigue siendo la representación interoperable e inspeccionable.
- La precisión del mapa se prueba con errores generados; no se reconstruyen ubicaciones a partir de texto de terminal.

## Evidencia

El estado y las tareas de la Fase 5 están en [roadmap](../roadmap.md#fase-5-primer-compilador-funcional); las reglas generales del flujo están en [pipeline](../compiler-pipeline.md).
## Evolución de la implementación

En Fase 9, el backend añadió `rustfmt` 1.99.1 (edición 2024) después de `prettyplease` y antes de exportar. La matriz verifica igualdad carácter por carácter con esa salida para las producciones estructurales documentadas. Esto conserva la decisión de generar desde tokens y AST, sin concatenar texto de entrada. El contrato específico entre formato `.ox` y Rust generado quedó formalizado en [ADR-0008](0008-contrato-de-formato.md).
