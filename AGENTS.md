# Instrucciones para agentes de Óxido

## Fuentes de verdad

- `plan` contiene la especificación del lenguaje; `ARCHITECTURE_RULES.md` es el contrato arquitectónico; `docs/roadmap.md` organiza fases y `docs/a-futuro.md` contiene objetivos, no capacidades implementadas.
- El código y las pruebas existentes determinan el estado real. No afirmes que una característica funciona solo porque aparece en los documentos.
- Los antiguos documentos de Payvia se retiraron de este proyecto cuando imponían dominios o infraestructura ajenos a Óxido. Los prompts de calidad y observabilidad que permanecen son guías técnicas condicionales: solo aplican sus apartados cuando el componente existe o está aprobado en la especificación.

## Arquitectura y calidad

- Implementa el compilador y sus herramientas centrales en Rust siguiendo `ARCHITECTURE_RULES.md`: aplica DDD, Clean Architecture, Hexagonal y SOLID a los límites reales de la toolchain, sin inventar entidades de negocio ni capas artificiales. Añade crates solo cuando tengan una responsabilidad real.
- Conserva una sola semántica Rust. No declares compatibilidad sin pruebas; rechaza traducciones ambiguas y conserva evidencia reproducible.
- Usa mocks y dobles para aislar pruebas unitarias cuando ayuden. No los uses para sustituir pruebas de integración o de extremo a extremo que deban comprobar el comportamiento real.
- Mide antes de optimizar. Protege tiempo de análisis, memoria, latencia del LSP y tamaño de entradas con límites y benchmarks pertinentes; no inventes objetivos de rendimiento sin medición.
- Revisa dependencias, licencia, vulnerabilidades y reproducibilidad de acuerdo con la etapa y el uso real del proyecto.

## Seguridad y privacidad

- Trata `.ox`, `oxido.toml`, dependencias, macros y solicitudes LSP como entradas no confiables. Sigue el modelo de amenazas y controles definidos en `plan`.
- No generes Rust concatenando texto no confiable. No invoques procesos mediante shell ni construyas comandos desde entradas del usuario. Valida y restringe rutas y escrituras al workspace.
- El compilador debe prohibir `unsafe` salvo excepción revisada y documentada. Maneja entradas malformadas sin pánicos y aplica límites de tamaño, anidamiento y tiempo.
- No compiles ni actives análisis que ejecute código en workspaces no confiables. Respeta Workspace Trust de VS Code. El playground, si se construye, requiere aislamiento y límites de recursos.
- No envíes telemetría por defecto. No filtres secretos, datos personales ni rutas absolutas en logs o informes.

## Trabajo y verificación

- Inspecciona el estado, las instrucciones y los cambios preexistentes antes de editar. Mantén los cambios acotados y conserva el trabajo del usuario.
- Ejecuta las verificaciones pertinentes al cambio cuando estén autorizadas y disponibles. Informa exactamente cuáles se ejecutaron, cuáles fallaron y cuáles no se ejecutaron.
- No ocultes diagnósticos ni debilites pruebas para conseguir un resultado verde. No declares completa una fase con criterios pendientes.
- En el código interno de Óxido no silencies warnings ni resultados: no uses `#[allow(unused_*)]`, `#[expect(unused_*)]` ni `let _ =` para descartar valores/errores. Maneja los resultados o reporta explícitamente el fallo. Los fixtures `.ox` pueden probar esas construcciones como sintaxis del usuario.
- No publiques, despliegues, cobres, ejecutes migraciones destructivas ni borres datos sin autorización explícita.
- Mantén `README.md` como orientación general y coloca especificaciones detalladas en los documentos propietarios existentes; evita duplicar documentación.
- `docs/` es la fuente canónica de especificaciones e informes. `website/src/content/docs/referencia/` y `audiencias/` son artefactos generados: edita los Markdown fuente, nunca las copias que prepara el build.
