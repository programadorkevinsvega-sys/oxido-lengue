# Roadmap de brechas verificadas

**Estado: propuesta parcialmente aprobada.** R1.6 está aprobada para preparar la publicación automática de paquetes Debian en GitHub Releases al crear una etiqueta de versión; el resto de esta propuesta no autoriza iniciar código, releases ni despliegues. Cada subfase se implementa en un PR independiente y pasa las pruebas de esta fase y de todas las anteriores antes de fusionarse.

## Auditoría de las brechas recibidas

| ID | Estado comprobado en el repositorio | Evidencia y decisión |
|---|---|---|
| B1 — Playground | Parcial: sandbox Linux local; servicio público no desplegado | `tools/playground` usa `bwrap`/`prlimit`, desactiva red y limita recursos. El README y [preparación empresarial](enterprise-readiness.md) dicen que no hay autenticación, cuotas ni despliegue. Planificar preparación, sin desplegar. |
| B2 — Binarios CLI publicados | Parcial | `debian-release.yml` prepara paquetes nativos amd64/arm64 y los adjunta a una GitHub Release al crear una etiqueta `v` coincidente con `oxido-cli`; la primera publicación depende de crear esa etiqueta. No cubre aún binarios CLI para Windows/macOS ni firma. |
| B3 — VSIX multiplataforma | Parcial | El workflow de release declara Linux x64, Windows x64 y macOS arm64; la evidencia publicada solo confirma ejecución host instalada en Linux x64. La matriz Windows/macOS no está demostrada. |
| B4 — Reproducibilidad entre sistemas | Abierta | El doble build con SHA-256 está verificado en Linux x86_64/Rust 1.99.1. No se ha archivado evidencia equivalente de Windows y macOS. La comparación debe ser entre builds del mismo target y entorno; binarios de sistemas distintos no deben tener el mismo hash. |
| B5 — Firma, notarización y Marketplace | Abierta; preparación solamente | El workflow excluye firma, notarización y publicación. Guías/scripts se pueden preparar después de aprobar; usar credenciales o publicar exige autorización separada. |
| B6 — Mensajes de `rustc` | Parcial | Doce códigos tienen resúmenes traducidos en CLI; el código y el detalle disponible se conservan, los desconocidos quedan originales y no existe selector de idioma. La ejecución real de E0308 y los tests golden están documentados en [diagnósticos](diagnostics.md). |
| B7 — Árboles `.ox` multiarchivo | Resuelta para el alcance probado | La resolución recursiva, exportación y compilación desde raíz, módulos `.ox`/`.rs`, mapas por archivo e importación de workspaces están implementados. Evidencia: `compiles_runs_and_recursively_exports_a_hierarchical_ox_project`, `resolves_path_modules_and_exports_their_rewritten_rust_paths` e `imported_workspace_compiles_through_oxido_with_local_member_dependencies`, descritas en [trazabilidad](../tests/trazabilidad.md). No se propone repetir esta implementación; compatibilidad arbitraria sigue delimitada por la matriz. |
| B8 — Sandbox general de dependencias | Abierta | La CLI/LSP no aíslan compilaciones con `build.rs` o macros procedurales. El sandbox del playground es independiente y no protege esos flujos. |
| B9 — Piloto educativo | No ejecutada; depende de terceros | El protocolo e instrumentos existen en [pilotos educativos](pilotos-educativos.md), pero no hay centro participante, datos ni análisis. No se puede cerrar mediante cambios del agente sin autor y participantes. |

## Reglas comunes para aprobar cada subfase

Cada PR de implementación debe incluir, según corresponda:

1. Pruebas unitarias y de lógica para cada campo, opción, límite y ruta de error tocados.
2. Prueba E2E contra el binario/servicio real pertinente; los dobles no reemplazan el flujo integrado.
3. Regresiones de defectos previos y suites de la fase actual y de las fases cerradas.
4. `cargo fmt --check`, `cargo check --workspace --all-targets --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`, `cargo audit` y build/enlaces del sitio cuando la documentación cambie.
5. Fuzzing y casos de tamaño/profundidad/Unicode cuando cambien parsers o superficies de entrada; pruebas diferenciales frente a Rust cuando cambie semántica/generación; benchmark solo si el cambio afecta rendimiento, con entorno y resultados registrados.
6. Checklist de aceptación con comando, resultado y enlace al test. Sin pruebas remotas verdes no se fusiona a `main`.

## Fase R1 — Evidencia multiplataforma y artefactos

### R1.1 — VSIX instalado en Windows x64

- **PR:** añadir/ajustar el job de host instalado en Windows, incluyendo el servidor empaquetado.
- **Aceptación:** el VSIX se instala en perfil limpio, activa desde su directorio instalado, resuelve `oxido-lsp.exe` sin depender de `PATH`, publica un diagnóstico `.ox` y registra SHA-256.
- **Pruebas:** manifiesto/paquete; prueba host real; caso negativo de binario ausente o no ejecutable; CI Windows verde.

### R1.2 — VSIX instalado en macOS arm64

- **PR:** ejecutar el mismo contrato host sobre macOS arm64.
- **Aceptación:** instalación y activación reales, servidor incluido ejecutable, diagnóstico `.ox` y SHA-256 archivado.
- **Pruebas:** paquete/manifiesto; host real; rechazo de servidor incompatible; CI macOS verde.

### R1.3 — Reproducibilidad independiente por sistema

- **PR:** ampliar el recolector para Linux, Windows y macOS con dos builds por sistema, mismo target/toolchain y directorios limpios.
- **Aceptación:** cada sistema obtiene hashes repetidos idénticos y adjunta informe con OS, arquitectura, toolchain, commit y comandos. No se exige igualdad de hash entre OS distintos.
- **Pruebas:** unitarias del parser del reporte; doble build E2E por runner; fallos negativos al alterar un artefacto; higiene de informes sin rutas absolutas.

### R1.4 — Paquetes descargables de CLI y LSP

- **PR:** preparar matriz de binarios CLI/LSP para Linux, Windows y macOS y checksums; el job produce artefactos sin crear/publicar releases.
- **Aceptación:** cada artefacto ejecuta `--version`, checksums se verifican, nombres/targets son inequívocos y los artefactos no incluyen secretos ni rutas locales.
- **Pruebas:** paquetes en runners reales; verificación de checksum válida e inválida; smoke de CLI/LSP; comparación con instalación desde fuente.

### R1.5 — Guía de instalación binaria

- **PR:** documentar descarga, verificación SHA-256, instalación, actualización y desinstalación para los targets probados.
- **Aceptación:** instrucciones reproducidas desde un runner limpio y limitaciones por arquitectura indicadas; sin prometer firma mientras no exista.
- **Pruebas:** enlaces/build del sitio y smoke de los comandos de instalación; revisión de cada URL/asset contra los nombres del workflow.

### R1.6 — Publicación directa de paquetes Debian

- **Estado:** aprobada; implementada en el PR de esta subfase y pendiente de CI/fusión.
- **PR:** construir `.deb` en runners nativos amd64 y arm64; instalarlos y ejecutar la CLI; agregar `SHA256SUMS`; adjuntar paquetes y checksums a una GitHub Release únicamente al empujar una etiqueta `v` igual a la versión de `oxido-cli`.
- **Aceptación:** CI valida ambos paquetes y la integridad de checksums; etiquetas de versión no coincidentes fallan; las ejecuciones de PR/manual no publican Releases; la publicación usa permisos `contents: write` limitados al job de release.
- **Pruebas:** smoke de checksum correcto/alterado; instalación/ejecución/eliminación del paquete real en cada arquitectura; workflow de PR exitoso; revisión de assets contra `SHA256SUMS`. La publicación real se activa solo con una etiqueta aprobada por el autor.

## Fase R2 — Diagnósticos de `rustc`

### R2.1 — Corpus golden para los códigos existentes

- **PR:** fijar entradas JSON reales o fixtures versionados para los doce códigos actuales, desconocidos, mensajes con/sin notas, spans primarios/secundarios y fuentes externas.
- **Aceptación:** el resumen traducido nunca elimina el código, notas ni mapeo; los desconocidos conservan el mensaje original.
- **Pruebas:** unitarias por código y variante; CLI E2E para ejemplos representativos; comparación del JSON estable y ubicación `.ox`.

### R2.2 — Selector explícito de idioma

- **PR:** proponer e implementar un selector documentado para salida CLI/JSON y LSP sin cambiar el mensaje original de `rustc` almacenado como dato relacionado.
- **Aceptación:** español/inglés producen resúmenes deterministas; el modo por defecto sigue especificado; códigos aún no traducidos se marcan como originales.
- **Pruebas:** golden por idioma, códigos soportados/desconocidos, JSON, salida humana y diagnostics LSP con span UTF-16.

### R2.3 — Ampliación basada en evidencia

- **PR:** agregar traducciones solo por familias con corpus real y revisión; no ampliar por conjetura.
- **Aceptación:** cada código nuevo tiene traducción validada, código/notas intactos, test positivo y negativo, y docs generadas desde la misma fuente de verdad si esa automatización se aprueba.
- **Pruebas:** corpus golden completo, ejecución CLI real y verificación de `diagnostics.md`, `compatibility.md` y `vscode.md`.

## Fase R3 — Diseño y aislamiento general de compilaciones

### R3.1 — Modelo de amenazas y ADR

- **PR:** documentar ataques de `build.rs`, macros procedurales, Cargo, toolchain, symlinks, filesystem, red, procesos hijos y agotamiento de recursos.
- **Aceptación:** decidir límites, aislamiento, compatibilidad por sistema, política de dependencias y comportamiento cuando el sandbox no exista. Sin código de ejecución en esta subfase.
- **Pruebas:** checklist de amenazas contra fixtures actuales y revisión de coherencia con Workspace Trust/política de dependencias.

### R3.2 — Prototipo de worker aislado

- **PR:** solo tras aprobar R3.1, aislar compilación de dependencias con filesystem temporal, red bloqueada, límites de CPU/memoria/tiempo/procesos y cierre de descendientes.
- **Aceptación:** fallar cerrado si faltan garantías; el host y sus rutas no quedan escribibles; capturar salida sin rutas absolutas.
- **Pruebas:** unitarias de configuración, E2E con fixtures benignos y hostiles de `build.rs`/proc-macro, timeout, memoria, fork, symlinks y prueba explícita de red inaccesible; fuzz de protocolo/configuración.

### R3.3 — Integración con CLI y editor

- **PR:** conectar el worker aprobado en `compilar`/`ejecutar`/`probar` y en la acción Cargo del editor, manteniendo Workspace Trust.
- **Aceptación:** modo seguro por defecto, mensajes/códigos claros, cancelación, sin ejecutar en workspace no confiable y mecanismo documentado cuando el sistema no soporte aislamiento.
- **Pruebas:** CLI y host VS Code reales, negativos de confianza, cancelación de procesos, dependencias con build script/proc-macro, `--locked --offline` y regresiones completas.

## Fase R4 — Preparación de despliegue del playground

El playground tiene sandbox local Linux; esta fase estudia un servicio distinto. Cloudflare Pages no ejecutará código.

### R4.1 — Diseño operativo y amenazas

- **PR:** proponer contenedor/VM, límites por usuario/IP, cuotas, concurrencia, colas, retención, observabilidad sin telemetría del compilador, costos y respuesta a abuso.
- **Aceptación:** decisión arquitectónica y amenazas revisadas; sin credenciales ni servicio expuesto.
- **Pruebas:** escenarios de abuso y cálculo reproducible de límites; no se mide rendimiento sin un entorno aprobado.

### R4.2 — Imagen reproducible y entorno local

- **PR:** preparar Dockerfile/imagen mínima y guía local, sin publicar ni desplegar.
- **Aceptación:** build reproducible, usuario no root, capacidades reducidas, filesystem efímero, red de ejecución bloqueada, límites y apagado limpio.
- **Pruebas:** inspección de imagen, E2E dentro del contenedor, prueba de escape/red/timeout/recursos y comparación con el sandbox Linux existente.

### R4.3 — Controles de cuota y abuso

- **PR:** implementar cuotas/rate limiting solo después de aprobar arquitectura/operación.
- **Aceptación:** límites por identidad definidos, respuestas estables al exceso, ninguna ejecución sin cuota y control del costo máximo.
- **Pruebas:** unitarias de ventana/límite, concurrencia, E2E de abuso y carga con resultados medidos. No habilitar endpoint público.

### R4.4 — Preparación de despliegue

- **PR:** preparar manifiestos, health checks, rollback, logs higienizados y runbook.
- **Aceptación:** validación local/CI y variables documentadas; publicación, despliegue o apertura de red requieren autorización explícita posterior.
- **Pruebas:** despliegue efímero local, verificación de health/readiness, rollback simulado, secretos ausentes en logs.

## Fase R5 — Preparación de firma y publicación

### R5.1 — Verificación de procedencia y checksums

- **PR:** documentar y preparar manifiestos de artefactos, hashes y verificación de procedencia sin subir binarios a un release.
- **Aceptación:** artefacto alterado falla; commit, target y toolchain quedan identificados.
- **Pruebas:** validación positiva/negativa de hashes y manifiestos en CI.

### R5.2 — Firma Windows/macOS

- **PR:** preparar scripts y workflow con secrets referenciados, pero no configurados ni ejecutados.
- **Aceptación:** dry-run sin credenciales demuestra rechazo seguro; guía dice qué credenciales configura el autor y dónde.
- **Pruebas:** validación estática del workflow y pruebas sin secretos; ninguna firma real.

### R5.3 — Notarización y Marketplace

- **PR:** preparar pasos, metadatos, checklist y validación local de paquetes.
- **Aceptación:** empaquetado listo, pero sin notarizar, publicar ni utilizar tokens del autor.
- **Pruebas:** manifest/package tests y validación de workflow; no release ni publicación.

## Fase R6 — Piloto educativo

### R6.1 — Preparar piloto con una institución

- **Responsable:** autor e institución participante; el agente puede mantener el kit tras aprobación.
- **Aceptación:** consentimiento, alcance, protocolo, protección de datos y criterio de análisis aprobados antes de recolectar datos.
- **Pruebas/evidencia:** revisión del kit, ejecución técnica de Libro #1–#3 en entorno aislado; sin afirmar resultados educativos.

### R6.2 — Ejecutar y reportar resultados

- **Responsable:** autor/equipo e institución, con autorización específica.
- **Aceptación:** informe con método, muestra, resultados y límites; datos personales no se publican y no se generalizan resultados de una muestra pequeña.
- **Pruebas/evidencia:** reproducción de análisis, revisión de privacidad y verificación de que toda afirmación tiene datos fuente.

## Puerta de aprobación

No se inicia ninguna fase R1–R6 con código hasta que el autor apruebe el alcance y el orden. La aprobación de una subfase no autoriza firma, notarización, Marketplace, publicación de releases ni despliegue público: esas acciones requieren autorización explícita separada y credenciales del autor. B7 queda fuera del backlog salvo que aparezca un caso nuevo que contradiga las pruebas listadas.
