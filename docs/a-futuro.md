# Visión futura de Óxido

Este documento reúne objetivos de producto para incorporar gradualmente. Son propuestas, no afirmaciones de capacidades ya implementadas. La compatibilidad se declara solo con evidencia en la matriz y pruebas del proyecto.

## 1. Prioridades de producto

### 1.1. Traducción visual Óxido ↔ Rust

Mientras se edita un `.ox`, VS Code podría mostrar el Rust generado y relacionar las construcciones equivalentes. El código generado debe poder inspeccionarse para aprender y auditar la traducción.

### 1.2. Diagnósticos útiles en español e inglés

Relacionar diagnósticos de Óxido, Cargo y `rustc` con el archivo `.ox` mediante el mapa de posiciones. Explicar errores sin ocultar el diagnóstico original ni inventar causas. Ofrecer sugerencias solo cuando puedan verificarse con seguridad.

### 1.3. Aprendizaje progresivo

Ofrecer ejercicios y explicaciones que introduzcan variables, funciones, condiciones, tipos, referencias, ownership, borrowing y concurrencia. La experiencia conserva las reglas de Rust y ayuda a entenderlas; no las elimina.

### 1.4. Compatibilidad verificable

Mantener una matriz por versión de Óxido con características soportadas, parciales y pendientes, fixtures, proyectos de referencia y versiones de Rust comprobadas. Una característica se marca como soportada solo tras superar sus criterios de aceptación.

### 1.5. Interoperabilidad con Rust

Usar crates, Cargo y pruebas existentes sin imponer un runtime separado. La salida `.rs` será inspeccionable y exportable. El flujo `.ox → .rs → .ox` debe delimitar el subconjunto reversible y documentar las pérdidas posibles.

### 1.6. Formato y migración de estilos

Un formateador conserva el modo elegido. Una herramienta explícita podrá migrar entre indentación y llaves si verifica que no cambia la semántica. No se mezclan estilos dentro de un archivo.

### 1.7. Adopción empresarial

Priorizar configuración reproducible, políticas de dependencias, licencias, vulnerabilidades, auditoría de `unsafe` y reportes útiles. La preparación empresarial requiere evidencia, soporte y limitaciones documentadas; no se deduce de un ejemplo compilado.

### 1.8. Playground

Un entorno web permitirá escribir, comparar y ejecutar ejemplos. Como ejecuta código ajeno, requiere aislamiento, red deshabilitada y límites estrictos de recursos definidos por el modelo de amenazas.

### 1.9. Documentación bilingüe

Mantener documentación en español e inglés y compilar ejemplos en CI. Un ejemplo inválido debe fallar esa verificación o quedar marcado como ilustrativo.

### 1.10. Análisis del proyecto

Generar un informe de características de Rust usadas, compatibilidad conocida, dependencias y superficie de riesgo. Explicar los límites del análisis y no presentarlo como prueba de seguridad total.

### 1.11. Refactorización e índice incremental

Añadir renombrado seguro y un índice incremental persistente para el editor, de modo que la navegación y la clasificación de tokens no dependan solo de los documentos abiertos. El renombrado deberá rechazar cambios ambiguos o no demostrables y conservar la semántica Rust; el índice persistente necesitará invalidación por versión, límites de tamaño y pruebas de recuperación. Son objetivos futuros: Fase 11 cierra sin estas capacidades.

## 2. Óxido como puente de aprendizaje

Ejemplo de la experiencia buscada, sujeto a la gramática que se implemente:

```ox
sintaxis indentacion

funcion sumar(a: i32, b: i32) -> i32:
    retornar a + b
```

El editor podría mostrar:

```rust
fn sumar(a: i32, b: i32) -> i32 {
    return a + b;
}
```

Y explicar que `funcion` corresponde a `fn`, `:` abre un bloque indentado, `retornar` corresponde a `return` e `i32` mantiene el tipo de Rust. Este ejemplo expresa un objetivo de producto; no afirma que el compilador actual ya lo admita.

## 3. Contrato de compatibilidad

Cada versión publicada especificará:

- La versión y edición de Rust probadas.
- Las construcciones de Óxido soportadas, parciales y rechazadas.
- Los crates, macros y patrones de interoperabilidad verificados.
- Las pruebas de regresión y proyectos de referencia ejecutados.
- Las limitaciones conocidas y el formato de migración.

El contrato se actualiza con evidencia reproducible por versión. No se certifica compatibilidad por inferencia ni por un ejemplo aislado.
