# ADR 0007: Contrato de interoperabilidad con Rust

- **Estado:** aceptada
- **Fecha:** 2026-10-25
- **Contexto:** Óxido ofrece sintaxis española e indentada y genera Rust nativo. La cobertura de implementación y de pruebas todavía es parcial; esa cobertura no debe convertirse en una restricción permanente del lenguaje.

## Decisión

La meta de Óxido es interoperar con el ecosistema Rust sin exigir cambios en las bibliotecas externas. Óxido aporta la sintaxis propia, su análisis estructural, la resolución de módulos, la traducción y los mapas de origen. Cargo y `rustc` conservan sus responsabilidades de resolución de dependencias, expansión de macros, análisis semántico, comprobación de tipos y compilación nativa.

1. **Sintaxis y semántica:** el frontend traduce la estructura de Óxido y preserva los elementos Rust admitidos. No crea semántica paralela para tipos, traits, borrowing o ejecución. Si una construcción no puede preservarse fielmente ni transformarse con una regla definida, produce un diagnóstico explícito; nunca la descarta ni la altera silenciosamente.
2. **Macros:** admitir macros Rust significa preservar sus invocaciones, atributos y árboles de tokens para que Cargo/rustc los procese; no significa implementar en Óxido un sistema que defina, analice o expanda macros Rust. Las bibliotecas externas se consumen sin modificar su fuente. `macro_rules!` y macros locales escritos como Rust se preservan como Rust. Los aliases de Óxido solo transforman los casos allowlisted; las DSL de macros externas permanecen opacas.
3. **Asincronía:** Óxido genera `async`/`await` Rust equivalente. La aplicación selecciona Tokio, async-std, futures u otro runtime compatible. Óxido no implementa un runtime alternativo.
4. **`unsafe`:** Óxido conserva la semántica Rust y permite APIs/dependencias que usan `unsafe`. La auditoría es optativa y no bloquea compilaciones salvo que el usuario active expresamente el modo de rechazo.
5. **Evidencia:** la compatibilidad se informa por construcción, versión, dependencia y prueba. Una prueba de un corpus inicial no autoriza afirmar compatibilidad general. La falta de cobertura se registra como trabajo de conformidad, no como no objetivo permanente.

## Verificación requerida

La Fase 8 cerró los criterios de interoperabilidad para los casos documentados en [su informe](../fase-8-integracion-rust.md): crates externos, traits/genéricos/lifetimes, macros declarativas/procedurales, DSL, async, API `unsafe` y diagnósticos. La evidencia cubre versiones fijadas y un corpus concreto, no compatibilidad general. La Fase 10 puede aportar infraestructura común de laboratorio y reportes; cualquier nueva afirmación de compatibilidad requiere una prueba propia.

## Consecuencias

- El frontend puede usar AST y tokens, pero cualquier normalización debe tener reglas explícitas y pruebas que detecten cambios de delimitadores, nombres y tokens.
- Las pruebas deben separar generación fiel, compilación `rustc` y comportamiento ejecutado.
- La confianza/sandbox de código de dependencias es un control de seguridad de la herramienta; no reduce el objetivo de compatibilidad del lenguaje.
- La matriz de compatibilidad describe evidencia actual y brechas. Esta ADR define la dirección normativa.
