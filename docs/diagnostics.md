# Diagnósticos y posiciones

## Representación

Todo diagnóstico interno usa la estructura `Diagnostic` con:

- `code`: identificador estable `OX` + categoría + secuencia.
- `severity`: `error`, `warning`, `info` o `hint`.
- `message`: mensaje principal legible. Los mensajes propios actuales están en español; selector/traducción al inglés queda pendiente de la capa CLI/LSP.
- `primary_span`: rango primario half-open `[start,end)` en bytes UTF-8 de `.ox`.
- `labels[]`: rangos secundarios y explicación corta.
- `notes[]`: contexto, incluyendo edición/modo si aplica.
- `suggestions[]`: reemplazo y rango solo si la corrección es segura.
- `related_rust`: código y mensaje de Cargo/rustc cuando el origen sea compilación.

Los offsets internos son bytes UTF-8. El render CLI muestra línea/columna 1-based. LSP convierte a la codificación de posiciones negociada; si no se negocia otra, UTF-16 según LSP.

## Códigos iniciales

| Código | Significado |
|---|---|
| `OX0001` | Configuración inválida |
| `OX0002` | Modo de archivo contradice configuración del proyecto |
| `OX0003` | Falta modo explícito |
| `OX1001` | Carácter/token inválido |
| `OX1002` | Control Unicode bidireccional rechazado |
| `OX1003` | Identificador visualmente confundible (warning) |
| `OX2001` | Indentación inconsistente |
| `OX2002` | Delimitador de bloque incompatible con el modo |
| `OX2003` | Se esperaba cuerpo después del encabezado de bloque |
| `OX2004` | Continuación multilínea inválida |
| `OX3001` | Keyword usada como identificador; sugerir `r#...` |
| `OX4001` | Construcción reservada, no soportada o no transformable en este modo |
| `OX5001` | Error interno de generación/parseo del Rust generado |
| `OX5002` | Diagnóstico Rust no pudo mapearse al origen `.ox` |
| `OX6001` | Límite de tamaño/profundidad/tiempo excedido |
| `OX7001` | Falta `Cargo.lock` para dependencias del proyecto; revisar el plan de `oxido descargar-dependencias` |
| `OX7002` | La compilación de dependencias requiere confianza explícita del operador |

Los errores de Rust conservan su código (por ejemplo `E0308`) y se presentan junto con el mensaje de Óxido. No se traducen ni esconden errores sin conservar el original.

## Mapa de posiciones

El mapa registra segmentos de origen `.ox` y destino `.rs`. Tokens emitidos conservan el span de origen; llaves, comas y terminadores sintéticos apuntan al encabezado/línea que los produjo. Diagnósticos de Cargo se leen como JSON `compiler-message`; usar `file_name`, `byte_start`, `byte_end`, líneas/columnas y spans secundarios. Solo se traducen rutas pertenecientes a los artefactos generados de Óxido; errores de dependencias externas conservan su ruta externa.

Cargo emite mensajes JSON por línea e incluye diagnósticos del compilador dentro de `compiler-message`; `rustc` spans usan offsets de bytes 0-based start-inclusive/end-exclusive ([Cargo External Tools](https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages), [rustc JSON diagnostics](https://doc.rust-lang.org/rustc/json.html#diagnostics)).
