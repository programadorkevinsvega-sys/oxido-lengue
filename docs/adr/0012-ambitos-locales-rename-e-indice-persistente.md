# ADR 0012: ámbitos locales, renombrado e índice persistente

- **Estado:** aceptado
- **Fecha:** 2026-10-27
- **Contexto:** extensión 11.1 del editor/LSP

## Contexto

[ADR 0011](0011-cierre-fase-11-y-distribucion.md) cerró la Fase 11 con tres límites explícitos: el índice de semantic tokens no modelaba ámbitos locales, los archivos cerrados no participaban en la clasificación de cada edición y `rename` junto con un índice incremental persistente quedaban fuera del alcance. Esos límites producían dos problemas concretos:

1. Un parámetro o un `dejar`/`let` con el mismo nombre que una declaración del workspace podía recibir la clase de la declaración global.
2. La navegación sobre archivos cerrados dependía de reconstruir el índice en cada sesión y el editor no ofrecía un renombrado seguro que rechazara ambigüedad.

El worker de `rust-analyzer` ya existía para hover, completado, inlay hints y navegación en workspaces confiables. También existían los mapas de origen, pero solo con correspondencias de fragmentos completos, insuficientes para editar únicamente el identificador seleccionado. La distribución, la firma y la matriz multiplataforma siguen siendo responsabilidad de la Fase 12.

## Decisión

1. **Ámbitos locales acotados.** El LSP registra parámetros y enlaces `dejar`/`let`/`para`/`for`, junto con sus bloques léxicos, sin inferir tipos ni evaluar expresiones. El enlace más interno gana en caso de sombreado. Un identificador local se clasifica como variable o parámetro dentro de su ámbito; fuera de él vuelve a resolverse contra las declaraciones. Esto no convierte al índice en un sistema de tipos.
2. **Delegación en `rust-analyzer`.** En un workspace confiable, `textDocument/semanticTokens/full`, `textDocument/prepareRename` y `textDocument/rename` se consultan primero al analizador. Un token o una edición solo se traduce al `.ox` cuando sus extremos coinciden exactamente con una correspondencia token a token del mapa de origen. Los fragmentos completos, los rangos parciales y las URI sin mapa se descartan.
3. **Fusión conservadora de tokens.** El token delegado gana cuando ocupa la misma región que el local; el resultado local solo rellena huecos. Un error transitorio, una respuesta vacía o un fallo de mapeo conserva el resultado local. No se presenta esta fusión como resolución de tipos universal.
4. **Índice persistente fuera del workspace.** El índice de declaraciones vive en la caché del usuario (`OXIDO_CACHE_DIR`, `XDG_CACHE_HOME`, `LOCALAPPDATA` o `HOME/.cache`), nunca dentro de la raíz del proyecto. Se versiona por esquema y modo, se identifica por un hash estable de la raíz y se invalida por fecha de modificación, tamaño y hash del contenido. La reutilización exige que la fecha y el tamaño coincidan; un contenido distinto con la misma fecha se reindexa por hash.
5. **Límites y escritura atómica.** El escaneo se limita a 256 archivos `.ox`, 2048 entradas de directorio, 4 MiB por archivo y 16 MiB de caché. Se omiten enlaces simbólicos y los directorios `target`, `.git` y `node_modules`. La caché se escribe en un temporal del mismo directorio y se publica con `rename`; una caché con versión o modo distinto se ignora. Los errores se registran sin rutas absolutas.
6. **Renombrado con rechazo explícito.** `prepareRename` y `rename` se anuncian en las capacidades. El fallback local renombra un enlace dentro de su ámbito léxico o un símbolo global solo si existe una única candidata demostrable. Se rechazan los nombres ambiguos, los identificadores no válidos y las palabras reservadas. Si `rust-analyzer` devuelve un `WorkspaceEdit` con alguna URI o algún rango sin correspondencia exacta, se rechaza el edit completo en lugar de aplicar una parte.
7. **Workspace Trust se mantiene.** En Restricted Mode no se inicia `rust-analyzer`; solo se usan el índice local y los ámbitos locales. La caché en disco tampoco ejecuta código ni interpreta el contenido como instrucciones.
8. **Distribución sin cambios de fase.** Firma, notarización, publicación, SBOM, builds reproducibles y la matriz Windows/macOS continúan en Fase 12. La extensión 11.1 no se presenta como paquete publicado ni como soporte multiplataforma verificado.

Esta decisión amplía el punto 3 y sustituye el punto 4 de ADR 0011 para el alcance del editor. El cierre técnico y el traslado de la distribución registrados en ADR 0011 siguen vigentes.

## Consecuencias

- Un enlace local ya no hereda la clase de una declaración homónima fuera de su ámbito, y los tokens de archivos cerrados se resuelven sin recorrer el workspace en cada pulsación.
- La caché reduce el trabajo de arranque, pero sigue siendo un artefacto derivado: se puede borrar sin perder código y no se considera fuente de verdad.
- El renombrado delegado conserva la semántica de `rust-analyzer` solo en las regiones mapeadas. Una edición que cruce fragmentos opacos, macros o módulos sin correspondencia se rechaza; el usuario debe resolverla manualmente.
- El fallback local es deliberadamente conservador: prefiere no renombrar antes que elegir una candidata arbitraria.
- El índice no modela tipos, genéricos, rasgos ni resolución de expresiones. La precisión profunda sigue dependiendo del analizador en workspaces confiables.
- La caché se limita a un workspace concreto y a un esquema; cambiar la versión, el modo o el contenido invalida las entradas afectadas.

## Evidencia

- `cargo test -p oxido-lsp --offline`: 61 pruebas, incluidas las de ámbitos locales y sombreado, renombrado local por ámbito, rechazo de sombreado y nombres cualificados ajenos al enlace, rechazo de ambigüedad, rechazo de nombre reservado, semantic tokens con archivos cerrados y persistencia/invalidación de la caché.
- `cargo test -p oxido-rust-backend --offline`: 10 pruebas, incluida `maps_individual_fragment_tokens_for_precise_rename_ranges`, que exige correspondencias token a token para rangos de renombrado.
- `cargo test --workspace --offline`, `cargo fmt --all -- --check` y `cargo clippy --workspace --all-targets --offline` sin errores.
- `npm test --prefix editors/vscode-oxido`: 4 pruebas de manifiesto y empaquetado.
- Suite host real de VS Code 1.143.1: 7 casos. El caso confiable exige `prepareRename`/`rename` mapeados a `semantic_tokens.ox`, `indexFiles > 0`, una caché escrita o reutilizada (`indexCacheWritten === true` o `indexCacheReused > 0`) y el archivo de caché fuera del fixture; el caso Restricted Mode exige `prepareRename`/`rename` locales sin `rust-analyzer`. El runner comprueba que no aparece `workspace-index-v*` dentro del workspace.
- El informe canónico es [Fase 11](../fase-11-vscode-lsp.md); los límites de distribución permanecen en [Fase 12](../roadmap.md#fase-12-seguridad-y-calidad-avanzada).
