# Calidad de ingeniería para Óxido

## Propósito

Aplicar prácticas de calidad al compilador y sus herramientas: lexer, parser, AST, traductor, backend Rust, CLI, LSP, extensión y configuración del proyecto. `plan` es la fuente normativa de la especificación y el alcance del lenguaje; este documento fija controles de ingeniería y verificación.

No declares características soportadas sin evidencia en implementación y pruebas. Los resultados deben distinguir aprobado, fallido, bloqueado y no ejecutado.

## Auditoría antes de cambiar

- Inspecciona el estado del repositorio, instrucciones, código, dependencias, fixtures y verificaciones disponibles.
- Identifica la fase y los criterios de aceptación aplicables en `plan`.
- Conserva el trabajo existente y limita cada cambio al objetivo de la tarea.
- Comprueba compatibilidad, mantenimiento, licencia y necesidad antes de añadir dependencias.

## Pruebas

### Unitarias y de propiedades

Cubre reglas del lexer, parser, AST, normalización de alias, resolución de configuración, generación de Rust, mapeo de posiciones y diagnósticos. Incluye entradas válidas, inválidas, límites, Unicode, comentarios, cadenas, bloques anidados y código incompleto.

Usa dobles de prueba para aislar unidades cuando sea útil. No los uses para sustituir la verificación real de Cargo, `rustc`, el LSP ni flujos de extremo a extremo.

Añade pruebas de propiedades cuando existan invariantes claros, por ejemplo conservación de tokens protegidos, estabilidad del formateador y correspondencia de rangos de fuente.

### Integración y extremo a extremo

Verifica el flujo real pertinente: `.ox` → análisis → Rust estándar → Cargo/`rustc` → resultado esperado. Comprueba errores de compilación y diagnósticos, no solo compilaciones exitosas. Para el LSP, prueba solicitudes reales del protocolo y respuestas con rangos correctos.

Cada defecto reproducible se convierte en una regresión. Las fixtures deben describir casos de prueba del lenguaje y no presentarse como funcionalidades implementadas por sí mismas.

### Fuzzing y robustez

Fuzzea lexer, parser, configuración y decodificación de mensajes LSP con `cargo-fuzz`. Conserva los casos que revelen fallos. Entradas malformadas o excesivas deben producir diagnósticos y no pánicos, bloqueos ni escrituras fuera del workspace.

## Seguridad

Trata los archivos `.ox`, `oxido.toml`, solicitudes LSP y código de dependencias como entradas no confiables. Aplica el modelo de amenazas y los controles establecidos en `plan`.

- Prohíbe `unsafe` en el compilador salvo excepción justificada, revisada y documentada. Mantén una opción de política para rechazar `unsafe` en el Rust analizado.
- Rechaza controles Unicode bidireccionales y advierte sobre identificadores visualmente confundibles.
- Limita tamaño, anidamiento, tokens, concurrencia y tiempo de análisis.
- Genera Rust con tokens/AST, no concatenando texto de entrada.
- Invoca procesos con argumentos separados, nunca mediante shell. Canonicaliza rutas y restringe acceso al workspace.
- No ejecutes compilación ni análisis que pueda ejecutar código en workspaces no confiables. Respeta Workspace Trust.
- El playground, si se implementa, necesita aislamiento del sistema operativo, sin red y con límites de recursos.
- No expongas secretos, datos personales ni rutas absolutas en diagnósticos, logs o artefactos.

## Rendimiento y optimización

Mide antes de optimizar. Mantén benchmarks reproducibles para tiempo y memoria de lectura, lexing, parsing, generación de Rust, compilación incremental y respuesta del LSP. Usa corpus representativos del lenguaje y registra hardware, versión de Rust y configuración.

Conserva métricas locales o artefactos de CI para comparar duración por etapa, tamaño de entrada/salida, diagnósticos emitidos, uso máximo de memoria y latencia/cancelaciones del LSP. Añade métricas de caché solo cuando exista una caché real. No recolectes el contenido del código fuente para medir rendimiento.

No inventes presupuestos de latencia ni métricas. Establece umbrales cuando existan mediciones base y una necesidad de producto. Evita que mediciones opcionales ralenticen el camino normal. No añadas Prometheus ni un colector remoto al compilador local por defecto.

## Dependencias y reproducibilidad

- Versiona `Cargo.lock` para aplicaciones y utiliza `--locked` en CI.
- Ejecuta controles de formato, lints, compilación y pruebas pertinentes a la fase.
- Incorpora análisis de vulnerabilidades y licencias según el estado del proyecto.
- Mantén builds reproducibles; documenta herramientas y versiones necesarias.
- Las compilaciones de publicación deben ser verificables; prepara firma y SBOM antes de distribuir binarios.

## CI y cierre de trabajo

CI debe ejecutar las verificaciones correspondientes al cambio y preservar sus resultados. No ocultes fallos desactivando pruebas o debilitando aserciones. Si una comprobación no puede ejecutarse, registra el bloqueo y no la declares aprobada.

Una fase se cierra solo al satisfacer sus criterios de aceptación, actualizar la documentación afectada y reportar comandos y resultados reales, limitaciones y pendientes.
