# Auditoría del repositorio — Fase 0

**Fecha:** 2026-10-25  
**Resultado:** Fase 0 completada; el proyecto está en etapa documental.  
**Siguiente fase:** Fase 1 — Especificación.

## 1. Estado del repositorio

- Ruta de trabajo: `oxido-lengue`.
- Rama activa: `main`, alineada con `origin/main` en el commit `2c3ba27` al momento de la auditoría.
- El historial tiene dos commits: uno de documentación de Óxido y un commit previo cuyo mensaje es “Change greeting from 'Hello' to 'Goodbye'”.
- Cambios locales pendientes al auditar: `plan` modificado; `a futuro`, `AGENTS.md`, los dos prompts de calidad/observabilidad y `docs/roadmap.md` presentes como cambios locales o no versionados.
- El inventario versionado en HEAD contiene únicamente `plan` y `a futuro`.

Se preservaron todos los cambios locales; esta auditoría no los restablece ni los descarta.

## 2. Inventario técnico

| Área | Estado observado | Clasificación |
|---|---|---|
| Workspace y manifiestos Cargo | No hay `Cargo.toml` ni workspace | No existe |
| Toolchain | No hay `rust-toolchain.toml` | No existe |
| Dependencias y lockfile | No hay `Cargo.lock` ni dependencias declaradas | No existe |
| Código Rust del compilador | No hay archivos `.rs` | No existe |
| Código fuente Óxido | No hay archivos `.ox` | No existe |
| Lexer, parser y AST | Solo están descritos en `plan` | Propuesto, no implementado |
| Backend de generación Rust | Solo está descrito en `plan` | Propuesto, no implementado |
| CLI | Solo está descrita en `plan` | Propuesta, no implementada |
| Pruebas automatizadas | No hay directorios ni archivos de pruebas | No existe una suite ejecutable |
| Fixtures y laboratorio | Descritos en `plan` y `docs/roadmap.md` | Propuestos, no implementados |
| CI | No hay workflows ni configuración de CI | No existe |
| Extensión VS Code y LSP | Descritos en documentos | Propuestos, no implementados |
| Seguridad y cadena de suministro | Requisitos escritos en `plan` y `AGENTS.md` | Política documental, controles aún no implementados |
| Rendimiento y métricas | Objetivos documentados; no hay benchmarks | Pendiente de implementación |
| Documentación | `plan`, `a futuro`, `AGENTS.md`, prompts de calidad/observabilidad y roadmap | Parcial; falta documentación técnica formal de `docs/` |

## 3. Decisiones documentadas que guían Fase 1

1. Óxido traduce archivos `.ox` a Rust estándar y delega la semántica de tipos, ownership y compilación a Rust.
2. Se contemplan modos de indentación y de llaves, con un solo modo por proyecto.
3. La configuración del proyecto es normativa; una declaración de modo por archivo no puede contradecirla.
4. El alias propuesto para `crate` es `paquete`; se proponen identificadores crudos `r#nombre` para colisiones.
5. Los argumentos de macros se conservan como árboles de tokens opacos por defecto.
6. La expresión final sin `;` conserva el valor del bloque; con `;` el bloque vale `()`.
7. La decisión inicial de alcance es analizar estructura propia y preservar expresiones/tipos Rust como tokens, validándolos con `syn` cuando corresponda.
8. El backend debe generar Rust con tokens/AST, no concatenando texto de entrada.
9. Desde el primer compilador se requiere mapa de posiciones `.ox` ↔ `.rs`, diagnósticos JSON de Cargo y una prueba de ida y vuelta para el subconjunto reversible.
10. No se envía telemetría remota por defecto. Las mediciones de rendimiento son locales y reproducibles; Prometheus no se requiere para la CLI local.

Estas son decisiones escritas en documentos, no capacidades probadas ni una gramática formal terminada.

## 4. Huecos y riesgos

- **Especificación todavía incompleta:** las decisiones guía están registradas, pero faltan gramática formal, lista cerrada de alias por edición, tokens, precedencias, reglas de continuaciones y criterios detallados de compatibilidad.
- **Sin prueba end-to-end:** todavía no existe código que lea `.ox`, emita `.rs`, invoque Cargo y ejecute el resultado.
- **Sin evidencia de compilación:** no hay toolchain/dependencias configuradas; no se intentó compilar.
- **Sin pruebas disponibles:** no existe suite de pruebas en el repositorio; no se ejecutaron pruebas. No debe interpretarse como suite aprobada.
- **Documentación faltante:** aún no existen `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `LICENSE` ni las especificaciones técnicas bajo `docs/` requeridas por `plan`.
- **Alcance por priorizar:** la API REST de referencia aparece en `plan`, pero no es necesaria para probar el primer compilador. Se debe postergar hasta que el pipeline .ox → Rust esté implementado y se haya demostrado interoperabilidad suficiente.
- **Riesgos de seguridad identificados y aún sin controles ejecutables:** entrada hostil, límites de parser, Unicode bidireccional, rutas y symlinks, procesos de compilación, `build.rs`/macros procedurales, LSP y futuro playground.
- **Estado local no integrado:** existen cambios documentales locales y archivos no versionados. No se creó una rama nueva ni se modificó el historial.

## 5. Verificaciones realizadas

- Inventario de archivos del workspace y archivos versionados con `git ls-tree`/`git ls-files`.
- Estado, rama, referencias remotas e historial con `git status`, `git branch`, `git remote` y `git log`.
- Búsqueda de manifiestos Cargo, lockfile, toolchain, fuentes Rust/Óxido y archivos de pruebas.
- Lectura de instrucciones y documentos de especificación/roadmap.

**Pruebas, formato y compilación:** no ejecutados; no existe proyecto Cargo ni suite en este repositorio.

## 6. Resultado de Fase 0

La auditoría está completa como unidad documental: el estado real, las decisiones previas, los componentes inexistentes, los riesgos y la siguiente fase están identificados. No se declara implementada ninguna capacidad del lenguaje.

Fase 1 puede empezar sin bloqueo externo. Debe cerrar la gramática inicial, el alcance exacto del parser, alias/reservadas, semántica de bloques, compatibilidad, diagnósticos y criterios de aceptación antes de crear el workspace funcional.
