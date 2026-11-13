# Diagnósticos y posiciones

## Representación

Todo diagnóstico interno usa la estructura `Diagnostic` con:

- `code`: identificador estable `OX` + categoría + secuencia.
- `severity`: `error`, `warning`, `info` o `hint`.
- `message`: mensaje principal legible. Los diagnósticos propios están en español. Para un conjunto inicial de códigos de `rustc`, la CLI reemplaza el resumen por una frase en español y conserva el código y el detalle original disponible; los códigos desconocidos mantienen el mensaje original. No existe selector de idioma.
- `primary_span`: rango primario half-open `[start,end)` en bytes UTF-8 de `.ox`.
- `labels[]`: rangos secundarios y explicación corta.
- `notes[]`: contexto, incluyendo edición/modo si aplica.
- `suggestions[]`: reemplazo y rango solo si la corrección es segura.
- `related_rust`: código y mensaje de Cargo/rustc cuando el origen sea compilación.

Los offsets internos son bytes UTF-8. El render CLI muestra línea/columna 1-based. El LSP publica diagnósticos léxicos/estructurales al editar y convierte spans a UTF-16. En workspace confiable, una acción explícita de VS Code ejecuta Cargo y publica diagnósticos mapeados a `.ox`; no compila automáticamente al abrir o editar. La cobertura y los límites están en la [guía de VS Code y LSP](vscode.md).

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
| `OX3002` | Nombre de función/método del archivo coincide con alias; se conserva como método propio y se informa cómo escribir la llamada estándar Rust |
| `OX4001` | Construcción reservada, no soportada o no transformable en este modo |
| `OX5001` | Error interno de generación/parseo del Rust generado |
| `OX5002` | Diagnóstico Rust no pudo mapearse al origen `.ox` |
| `OX6001` | Límite de tamaño/profundidad/tiempo excedido |
| `OX7001` | Falta `Cargo.lock` para dependencias del proyecto; revisar el plan de `oxido descargar-dependencias` |
| `OX7002` | La compilación de dependencias requiere confianza explícita del operador |

Los errores de Rust conservan su código (por ejemplo `E0308`). La CLI traduce el resumen de los códigos iniciales `E0308`, `E0425`, `E0599`, `E0382`, `E0502`, `E0507`, `E0277`, `E0433`, `E0061`, `E0063`, `E0004` y `E0106`; los detalles de rustc que siguen al resumen se conservan. Los códigos fuera de la lista se muestran sin traducir. La regresión `translates_known_rustc_codes_and_preserves_unknown_messages` fija el comportamiento; una ejecución real de `oxido compilar --json` con una asignación `&str` a `i32` produjo `E0308`, «tipos incompatibles» y ubicación en el `.ox` de entrada. No hay selector de idioma.

## Mapa de posiciones

El mapa registra segmentos de origen `.ox` y destino `.rs`. Tokens emitidos conservan el span de origen; llaves, comas y terminadores sintéticos apuntan al encabezado/línea que los produjo. Diagnósticos de Cargo se leen como JSON `compiler-message`; usar `file_name`, `byte_start`, `byte_end`, líneas/columnas y spans secundarios. Solo se traducen rutas pertenecientes a los artefactos generados de Óxido; errores de dependencias externas conservan su ruta externa.

Cargo emite mensajes JSON por línea e incluye diagnósticos del compilador dentro de `compiler-message`; `rustc` spans usan offsets de bytes 0-based start-inclusive/end-exclusive ([Cargo External Tools](https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages), [rustc JSON diagnostics](https://doc.rust-lang.org/rustc/json.html#diagnostics)). La guía de [VS Code y LSP](vscode.md) describe la ruta explícita Cargo, el mapeo por archivo y sus pruebas host.
