# Visión futura de Óxido

Esta página separa el estado verificable de las propuestas. «Implementado» significa que existe una capacidad documentada; el alcance exacto sigue sujeto a las pruebas y límites enlazados. «Parcial» indica que una parte está disponible y otra sigue propuesta.

## Capacidades actuales relacionadas

| Tema | Estado | Alcance verificado y límite |
|---|---|---|
| Matriz de compatibilidad | Implementado | La [matriz](compatibility.md) publica el alcance probado y sus límites. Debe actualizarse con cada nueva evidencia; no promete compatibilidad general con Rust. |
| Interoperabilidad e importación/exportación | Parcial | La CLI genera, importa y compila árboles en el alcance descrito por la [matriz](compatibility.md). La compatibilidad general con cualquier crate/proyecto no está demostrada. |
| Formato de `.ox` | Implementado | `oxido formato` es idempotente, conserva comentarios en los casos cubiertos y valida el Rust exportado frente a `rustfmt`; la cobertura detallada está en [pruebas](testing.md). |
| Playground local Linux | Parcial | Existe un sandbox local con red deshabilitada y límites; no está desplegado como servicio público. No incluye autenticación ni cuotas. |
| Análisis de proyecto | Parcial | `auditar-proyecto` inventaría superficie de riesgo detectable. No es un análisis completo de compatibilidad ni una certificación de seguridad. |
| Índice y refactorización del editor | Parcial | Hay caché/indexación limitada y renombrado conservador con `rust-analyzer` o fallback local. No cubre universalmente macros, código opaco ni símbolos sin mapeo exacto. |

Estas filas describen capacidades existentes; no son propuestas para volver a implementarlas.

## Trabajo propuesto

| Tema | Estado | Trabajo pendiente |
|---|---|---|
| Vista del Rust generado junto al `.ox` | Propuesto | La exportación permite inspeccionar Rust desde la CLI. Falta una vista sincronizada dentro del editor que relacione ambas representaciones. |
| Diagnósticos en español e inglés | Parcial | Los diagnósticos propios y un conjunto inicial de resúmenes de `rustc` están en español; no hay selector de idioma y los códigos desconocidos conservan el mensaje original. Véase [diagnósticos](diagnostics.md). |
| Aprendizaje progresivo | Parcial | Hay [tutorial](tutorial.md), [ejercicios](ejercicios.md) y ejemplos ejecutables. No se ha evaluado el material con estudiantes/docentes ni se afirma un beneficio educativo. |
| Migración entre modos de sintaxis | Propuesto | No se ofrece una migración automática entre indentación y llaves. Si se añade, debe comprobar equivalencia antes de escribir. |
| Adopción empresarial | Parcial | Hay política de dependencias, auditorías, fuzzing e informes reproducibles. Óxido sigue en etapa temprana, no apto para producción ni certificado. Véase [preparación empresarial](enterprise-readiness.md). |
| Documentación bilingüe | Parcial | Se mantiene el quickstart en inglés; el resto del contenido canónico está en español y no tiene traducción inglesa completa. |

## Óxido como puente de aprendizaje

El tutorial y los ejemplos existentes muestran construcciones actualmente verificadas. Este ejemplo representa una posible forma de explicar la relación entre las sintaxis, no una capacidad adicional:

```ox
sintaxis indentacion

funcion sumar(a: i32, b: i32) -> i32:
    retornar a + b
```

```rust
fn sumar(a: i32, b: i32) -> i32 {
    return a + b;
}
```

La correspondencia entre `funcion`/`fn`, `retornar`/`return` y el bloque se puede consultar con la exportación de Óxido. Una vista sincronizada del editor sigue propuesta.

## Contrato de compatibilidad

La [matriz de compatibilidad](compatibility.md) y la [política de pruebas](testing.md) registran la versión de Rust, construcciones, dependencias, pruebas y limitaciones verificadas. Ninguna propuesta futura amplía ese alcance por sí sola.
