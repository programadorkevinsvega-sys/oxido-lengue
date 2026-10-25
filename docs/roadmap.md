# Roadmap de Óxido

Este roadmap desglosa las fases 0–14 de `plan` en unidades implementables. Es un plan de trabajo, no un informe de capacidades existentes.

## Estado inicial

**Fases 0–4: completadas en sus alcances definidos. Fase 5 en adelante: pendiente. El portal de documentación se inició transversalmente y se consolidará en Fase 13.** En el momento de la auditoría de Fase 0 aún no había `Cargo.toml`, código fuente ni pruebas ejecutables; las Fases 2–4 añadieron la base Rust, el lexer y el parser/AST inicial. La Fase 4 no genera ni compila Rust. Consulta [el informe de Fase 0](auditoria-fase-0.md) y los documentos normativos de lenguaje enlazados abajo.

## Reglas para avanzar

- Cerrar cada subfase con evidencia verificable, limitaciones registradas y documentación actualizada.
- No declarar compatibilidad por ejemplos ilustrativos; registrar funciones soportadas, parciales, rechazadas y no evaluadas.
- Mantener el laboratorio desde las primeras fases. Las fases 10 y 12 lo amplían; no posponen las pruebas iniciales.
- Las metas de seguridad, calidad y rendimiento aplican desde el inicio según el riesgo de cada componente. No requieren añadir servicios externos de telemetría.
- No añadir tecnologías de backend que no formen parte de Óxido. Las dependencias se justifican por una necesidad concreta del compilador, CLI, LSP o extensión.
- Una subfase puede dividirse en tareas más pequeñas; no se puede saltar un criterio de salida para empezar trabajo dependiente.
- El portal documental estático es una superficie del producto separada de VS Code: se construye desde Markdown en `website/`, se despliega como archivos estáticos y se actualiza en cada fase. Su contenido solo presenta como disponibles las capacidades verificadas.

---

## Fase 0. Auditoría del repositorio

**Objetivo:** conocer el estado real y separar especificación, código existente y propuestas.

### Subfases

1. **Inventario del repositorio:** revisar archivos, ramas, historial, instrucciones y documentación; identificar archivos ajenos o duplicados.
2. **Auditoría técnica:** localizar workspace Cargo, crates, dependencias, toolchain, CI, fixtures, pruebas, extensión y LSP existentes.
3. **Verificación de estado:** ejecutar solo las comprobaciones existentes y seguras; registrar comandos y resultados, incluidos fallos previos.
4. **Matriz de capacidades:** clasificar cada característica como implementada, parcial, pendiente, no soportada o no evaluada.
5. **Plan adaptado:** identificar riesgos, decisiones bloqueantes, dependencias y la unidad mínima end-to-end.

**Entregables:** informe de auditoría, matriz de estado y plan de trabajo basado en evidencia.

**Criterio de salida:** el estado del repositorio está descrito sin asumir que una propuesta documental ya funciona; los problemas existentes y la primera unidad verificable están identificados.

**Dependencia:** ninguna.

---

## Fase 1. Especificación

**Objetivo:** cerrar las decisiones que condicionan lexer, parser, backend y compatibilidad.

### Subfases

1. **Alcance de Rust:** definir el frontend inicial. Decisión de partida: Óxido analiza estructura, bloques, alias e items propios; conserva expresiones y tipos como tokens de Rust y los valida con `syn` cuando corresponda.
2. **Modos de sintaxis:** especificar gramática de indentación y llaves, selección explícita, continuaciones, tabuladores, espacios, dedentación y errores.
3. **Política por proyecto:** el proyecto fija el modo. Los archivos no pueden contradecirlo; sin configuración de proyecto, cada archivo declara su modo al inicio.
4. **Semántica de bloques:** formalizar expresión final sin `;` como valor y con `;` como instrucción de valor `()`; definir los casos de continuación multilínea.
5. **Alias e identificadores:** inventariar palabras clave por edición de Rust; usar `paquete` para `crate`; definir reservadas, identificadores crudos `r#...`, colisiones y diagnósticos.
6. **Macros y atributos:** conservar nombre y contenido como tokens opacos; Óxido 0.1 no ofrece alias de macros.
7. **Compatibilidad:** crear matriz de construcciones conservadas, transformadas, rechazadas y pendientes.
8. **Diagnósticos y posiciones:** definir códigos, severidad, rangos, etiquetas y contrato del mapa `.ox` ↔ `.rs`.
9. **Backend:** especificar generación con árboles de tokens/AST (`proc_macro2`/`quote`) y formato estructural (`prettyplease`), nunca por concatenación de texto no confiable.
10. **Criterios de aceptación:** preparar fixtures positivos y negativos para cada decisión formal.

**Entregables:** [especificación](language-spec.md), [gramática](grammar.md), [modos](syntax-modes.md), [alias y reservadas](aliases.md), [compatibilidad](compatibility.md), [diagnósticos y mapa](diagnostics.md), [pipeline](compiler-pipeline.md) y [criterios de aceptación](testing.md). La arquitectura y el scaffold documental están en [architecture.md](architecture.md) y [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md).

**Criterio de salida:** todas las decisiones arquitectónicas anteriores están cerradas y no hay reglas críticas que dependan de interpretación informal.

**Dependencia:** Fase 0.

---

## Fase 2. Infraestructura del monorepo

**Objetivo:** preparar una base Rust mínima que compile y pueda crecer modularmente.

### Subfases

1. **Workspace y toolchain:** crear `Cargo.toml`, `Cargo.lock`, versión de Rust y edición fijadas, solo después de aprobar la arquitectura.
2. **Crates iniciales:** materializar solo los límites reales de fuente, backend, aplicación y CLI. Lexer/parser propios se implementan en Fase 3; el pipeline inicial prueba el parser Rust real de Syn.
3. **Interfaces internas:** definir flujo de datos, errores y propiedad de rangos entre etapas.
4. **Configuración:** establecer carga segura de `oxido.toml` como datos declarativos y validación de campos.
5. **Calidad base:** formato, lints, pruebas mínimas y CI reproducible con `--locked`.
6. **Laboratorio inicial:** fixtures y harness que ejerciten parseo Rust, backend y configuración declarativa reales; agregar lexer Óxido al harness en Fase 3.

**Entregables:** workspace mínimo, CI base y laboratorio inicial del parser Rust/backend. No es aún un compilador de `.ox`.

**Criterio de salida:** workspace compila, interfaces no tienen dependencias circulares y una prueba verifica el pipeline real aunque todavía tenga alcance pequeño.

**Dependencia:** Fase 1.

---

## Fase 3. Lexer y fuentes

**Objetivo:** convertir texto fuente en tokens y rangos confiables.

### Subfases

1. **Modelo de fuentes:** archivos, spans, offsets, líneas, columnas y manejo consistente de UTF-8.
2. **Tokens básicos:** identificadores, palabras clave inglesas/españolas, puntuación y operadores.
3. **Literales y comentarios:** números, cadenas, caracteres, comentarios y casos multilínea.
4. **Identificadores crudos:** reconocer `r#nombre` y distinguirlo de keywords.
5. **Seguridad Unicode:** rechazar controles bidireccionales y advertir sobre caracteres confundibles según la política de Fase 1.
6. **Modo indentación:** stack de indentación, dedentación, espacios/tabuladores y errores con spans.
7. **Modo llaves:** delimitadores de bloque sin dar significado semántico a la indentación.
8. **Tokens opacos:** preservar correctamente los grupos de macros, literales y atributos pertinentes.
9. **Límites y recuperación:** tamaño máximo, anidamiento, entradas incompletas y ausencia de pánicos.

**Entregables:** lexer funcional y pruebas positivas/negativas por token y modo.

**Criterio de salida:** los mismos bytes de entrada producen tokens/rangos deterministas; errores malformados son diagnósticos y no pánicos.

**Dependencia:** Fase 2.

---

## Fase 4. Parser y AST

**Objetivo:** analizar el subconjunto definido y producir una representación intermedia común.

### Subfases

1. **AST inicial:** items, bloques, declaraciones y spans de origen.
2. **Items y funciones:** visibilidad, firmas y cuerpos según la gramática admitida.
3. **Expresiones y tipos Rust:** preservar tokens que pertenecen a Rust y validarlos con `syn` dentro del alcance acordado.
4. **Bloques:** asociar indentación o llaves con la misma estructura de AST.
5. **Semicolon/valor:** representar de forma explícita la expresión final y el terminador `;`.
6. **Macros/atributos:** conservar grupos y tokens sin traducir argumentos por defecto.
7. **Errores y recuperación:** spans precisos, errores múltiples cuando sea seguro y tolerancia a edición incompleta.
8. **Pruebas del parser:** corpus válido/inválido, anidamiento, Unicode, macros y casos de recuperación.

**Entregables:** [parser/AST inicial](fase-4-parser.md) para el subconjunto definido.

**Criterio de salida:** corpus válido/inválido cubre AST, spans, recuperación, macros y semicolon/tail; ambos modos comparten semántica interna. Cumplido para el alcance inicial, con límites registrados en el informe de Fase 4.

**Dependencia:** Fase 3.

---

## Fase 5. Primer compilador funcional

**Objetivo:** traducir un `.ox` real, compilarlo y ejecutarlo de forma reproducible.

### Subfases

1. **Backend de tokens:** emitir Rust estándar mediante `quote`/`proc_macro2`; formatear con `prettyplease`.
2. **Mapa de posiciones:** guardar relación de spans entre fuente `.ox` y archivo Rust generado.
3. **Integración Cargo:** preparar proyecto temporal o estructura de salida segura; invocar procesos mediante `Command` y argumentos separados.
4. **Diagnósticos Cargo:** leer `cargo --message-format=json` y mapear errores de Rust al origen `.ox`.
5. **CLI mínima:** implementar verificar/compilar/ejecutar para el subconjunto aprobado.
6. **Prueba end-to-end:** compilar y ejecutar un programa mínimo mediante Cargo y `rustc`.
7. **Exportar/importar:** exportar `.ox` a `.rs` e importar el subconjunto reversible; probar `.ox → .rs → .ox` y documentar pérdidas.

**Entregables:** programa `.ox` real compilado y ejecutado, Rust generado inspeccionable, diagnósticos mapeados y prueba de ida y vuelta del subconjunto reversible.

**Criterio de salida:** ejecución automatizada reproducible compara el resultado esperado; no hay éxito reportado si falla Cargo o `rustc`.

**Dependencia:** Fase 4.

---

## Fase 6. Sintaxis de bloques y bilingüismo

**Objetivo:** completar ambos modos y normalizar alias sin modificar tokens protegidos.

### Subfases

1. **Modo indentado:** completar reglas de bloques, continuaciones y terminadores.
2. **Modo de llaves:** completar bloques equivalentes y terminadores.
3. **Configuración:** validar modo del proyecto y declaración inicial coincidente por archivo.
4. **Alias:** normalizar tokens de keywords con pruebas por alias y colisión.
5. **Escape:** comprobar identificadores crudos y mensajes de palabra reservada.
6. **Macros:** validar `matches!`, macros externas y grupos anidados sin traducir sus argumentos.
7. **Equivalencia:** compilar pares de programas equivalentes y comparar resultados.
8. **Formato:** formatear según modo sin cambios silenciosos de estilo.

**Entregables:** matriz de sintaxis y bilingüismo con pruebas de equivalencia y regresión.

**Criterio de salida:** ningún archivo mezcla delimitadores de bloque; diferencias de comportamiento quedan identificadas y justificadas.

**Dependencia:** Fase 5.

---

## Fase 7. Lenguaje inicial

**Objetivo:** ampliar el subconjunto mediante capacidades con criterios independientes.

### Subfases

1. Variables, mutabilidad, literales y expresiones.
2. Funciones, parámetros, retorno y visibilidad.
3. Condiciones y bucles.
4. Tuplas, arrays y tipos primitivos.
5. Estructuras, enumeraciones y métodos.
6. Referencias y sintaxis de borrowing delegada a Rust.
7. Módulos e importaciones.
8. Manejo de errores y tipos comunes.
9. Colecciones y biblioteca estándar, manteniendo nombres de tipos/métodos Rust.
10. Pruebas diferenciales y matriz de soporte por característica.

**Entregables:** subconjunto inicial definido y matriz actualizada.

**Criterio de salida:** cada característica tiene pruebas positivas, negativas, generación inspeccionada y compatibilidad documentada.

**Dependencia:** Fase 6; algunas características pueden desarrollarse en paralelo si no comparten gramática/backend.

---

## Fase 8. Integración con Rust

**Objetivo:** ampliar compatibilidad e interoperabilidad gradualmente.

### Subfases

1. Dependencias Cargo reales y configuración reproducible.
2. Genéricos, traits y lifetimes dentro del alcance del frontend.
3. Pattern matching, closures e iteradores.
4. Macros declarativas y atributos, preservando árboles de tokens.
5. `async`/`await` y concurrencia según compatibilidad validada.
6. Bibliotecas externas de referencia y pruebas de conformidad.
7. `unsafe`: preservar semántica Rust; política opcional de rechazo y auditoría de superficie.
8. Actualizar matriz con límites por versión/edición de Rust.

**Entregables:** suites de conformidad y proyectos de referencia versionados.

**Criterio de salida:** ninguna característica se marca soportada sin pruebas con Rust/Cargo reales y limitaciones registradas.

**Dependencia:** Fase 7.

---

## Fase 9. CLI y experiencia de proyecto

**Objetivo:** entregar comandos estables y seguros para tareas habituales.

### Subfases

1. Definir nombres, argumentos, ayuda y códigos de salida.
2. Crear proyectos con plantillas mínimas verificables.
3. Implementar verificar, compilar, ejecutar y probar.
4. Implementar formato y modo de comprobación sin modificar estilo configurado.
5. Implementar diagnóstico y salida estructurada para CI.
6. Implementar exportación/importación del subconjunto reversible.
7. Implementar informe de superficie de riesgo (`unsafe`, `build.rs`, macros procedurales y llamadas externas detectables).
8. Endurecer rutas, enlaces simbólicos, configuración y ejecución de procesos.

**Entregables:** CLI documentada y pruebas de integración de comandos.

**Criterio de salida:** comandos producen efectos, salidas y errores consistentes; operaciones de archivo quedan dentro del workspace permitido.

**Dependencia:** Fases 5–8 según comando.

---

## Fase 10. Laboratorio de validación

**Objetivo:** consolidar pruebas, compatibilidad, fuzzing y evidencia continua.

### Subfases

1. Matriz de fixtures por lexer, parser, modos, alias, macros, backend y diagnósticos.
2. Pruebas de regresión de defectos previos.
3. Pruebas diferenciales frente a Rust para construcciones equivalentes.
4. Pruebas de ida y vuelta/exportación-importación.
5. Fuzzing con `cargo-fuzz`, corpus y regresiones.
6. Pruebas de tamaño, profundidad, Unicode, recuperación y cancelación.
7. Proyectos de referencia: CLI, API educativa solo si corresponde al ejemplo del lenguaje, y dependencias reales compatibles.
8. Informes reproducibles con versiones, commit, entorno, pruebas aprobadas/fallidas/pendientes y rendimiento medido.
9. Integración de suites en CI y detección de regresiones.

**Entregables:** laboratorio ejecutable y reportes sin métricas inventadas.

**Criterio de salida:** cada característica anunciada está cubierta por pruebas identificables; pendientes y fallos se muestran explícitamente.

**Dependencia:** el laboratorio base inicia en Fase 2; esta fase consolida capacidades avanzadas.

---

## Fase 11. VS Code y LSP

**Objetivo:** construir una experiencia de edición que comparte análisis con el compilador.

### Subfases

1. Extensión mínima: asociación de `.ox`, configuración y comandos.
2. Semantic tokens del lexer/parser de Óxido.
3. Servidor LSP en Rust con tolerancia a archivos incompletos.
4. Diagnósticos del compilador y Cargo con mapeo al `.ox`.
5. Integración con `rust-analyzer` para tipos/autocompletado sobre Rust generado cuando el mapeo sea fiable.
6. Navegación, símbolos, referencias y renombrado solo cuando puedan verificarse.
7. Formateador y acciones de código.
8. Workspace Trust: no ejecutar compilación/análisis que ejecute código sin confianza explícita.
9. Cancelación, límites de solicitudes, memoria y latencia.
10. Pruebas del protocolo y documentación de instalación.

**Entregables:** extensión/LSP probados contra compilador real.

**Criterio de salida:** los diagnósticos y tokens coinciden con el análisis del compilador; el editor permanece receptivo ante código incompleto y proyectos no confiables.

**Dependencia:** Fases 3–5; capacidades avanzadas dependen de Fases 6–9.

---

## Fase 12. Seguridad y calidad avanzada

**Objetivo:** reducir riesgos de parser, cadena de suministro y artefactos distribuidos.

### Subfases

1. Modelo de amenazas actualizado para `.ox`, configuración, dependencias/macros, LSP y playground.
2. Aplicar límites de tamaño, anidamiento, tokens, tiempo y frecuencia.
3. `#![forbid(unsafe_code)]` en el compilador y política de rechazo de `unsafe` en código analizado.
4. Detección de Unicode bidireccional y advertencias de caracteres confundibles.
5. Auditoría de rutas, symlinks, permisos y procesos hijos.
6. `cargo-audit`, `cargo-deny`, `--locked` y política de dependencias/licencias.
7. Fuzzing continuo, revisión de pánicos y límites de memoria/CPU.
8. Benchmarks de análisis, generación, memoria y latencia del LSP; optimizar según perfiles.
9. Builds reproducibles, SBOM y firma antes de distribución.
10. Informes CI sin secretos ni rutas absolutas; telemetría desactivada por defecto.
11. Sandbox de playground, solo si se aprueba ese producto.

**Entregables:** checklist de seguridad, resultados de auditoría y plan de corrección.

**Criterio de salida:** riesgos críticos corregidos o bloqueados/documentados; controles automáticos ejecutados y reproducibles.

**Dependencia:** controles base desde Fases 2–5; consolidación desde Fase 10.

---

## Fase 13. Educación y documentación

**Objetivo:** convertir capacidades verificadas en materiales confiables dentro del portal documental Markdown desplegable en Cloudflare Pages.

### Subfases

1. README, contribución, seguridad, licencia y changelog coherentes.
2. Especificación de lenguaje, gramática, alias, modos y diagnósticos.
3. Arquitectura, pipeline y compatibilidad con Rust.
4. Instalación, CLI, editor/LSP y solución de problemas.
5. Tutoriales progresivos con ejemplos que compilan en CI.
6. Ejercicios y criterios de evaluación.
7. Guías bilingües cuando exista capacidad de mantener ambas versiones.
8. Plan de pilotos educativos y evaluación con evidencia; no afirmar beneficios sin estudio.
9. Guía empresarial que enumera límites y controles reales, no aspiracionales.
10. Mantener `website/` con Astro Starlight: portada, navegación, búsqueda, páginas para desarrolladores/empresas/estudiantes y casos de uso.
11. Publicar la referencia desde los Markdown canónicos de `docs/` sin editarlos dos veces; verificar enlaces y build en CI.
12. Desplegar estáticamente en Cloudflare Pages con previews; configurar dominio al decidirlo, sin añadir backend ni telemetría.

**Entregables:** portal de documentación versionado, guías por audiencia, ejemplos vinculados a pruebas y despliegue estático.

**Criterio de salida:** portal accesible desde móvil/escritorio, navegación y búsqueda funcionan, build estático se reproduce, documentos coinciden con implementación y ejemplos pendientes se identifican como propuestas.

**Dependencia:** cada documento se actualiza durante su fase; consolidación depende de Fases 7–12.

---

## Fase 14. Estabilización y lanzamiento

**Objetivo:** publicar solo un alcance cerrado y reproducible.

### Subfases

1. Congelar el alcance de versión y matriz de compatibilidad.
2. Ejecutar suite completa, auditorías, fuzzing definido y proyectos de referencia.
3. Verificar instalación limpia y toolchain compatible.
4. Revisar seguridad, dependencias, licencias, SBOM, firma y reproducibilidad.
5. Confirmar que no hay regresiones bloqueantes ni criterios ignorados.
6. Finalizar changelog, notas de migración, limitaciones y soporte.
7. Preparar artefactos y procedimiento de publicación.
8. Publicar únicamente con autorización y evidencia de aceptación.

**Entregables:** candidato de versión reproducible, informe de aceptación y documentación de lanzamiento.

**Criterio de salida:** todos los criterios de la versión están satisfechos; cada excepción tiene dueño, justificación y estado explícito. Si hay bloqueos, no se declara el lanzamiento listo.

**Dependencia:** Fases 0–13 y aceptación formal del alcance.

---

## Seguimiento del roadmap

Mantén una tabla de estado en cada revisión del proyecto:

| Fase | Estado | Evidencia | Bloqueos |
|---|---|---|---|
| 0 | Completada | [Informe de auditoría](auditoria-fase-0.md); no había pruebas ejecutables | Ninguno para iniciar Fase 1 |
| 1 | Completada (documental) | Especificación y criterios normativos: [language-spec.md](language-spec.md), [grammar.md](grammar.md), [syntax-modes.md](syntax-modes.md), [aliases.md](aliases.md), [compatibility.md](compatibility.md), [diagnostics.md](diagnostics.md), [compiler-pipeline.md](compiler-pipeline.md), [testing.md](testing.md); arquitectura: [ARCHITECTURE_RULES.md](../ARCHITECTURE_RULES.md) y [architecture.md](architecture.md) | Ninguno para iniciar Fase 2; aceptación de implementación aún pendiente |
| 2 | Completada | [Informe de Fase 2](fase-2-infraestructura.md), workspace, cuatro crates, parseo seguro de configuración, CI y pruebas del pipeline Rust inicial | El parser/AST `.ox` pertenece a Fase 4 |
| 3 | Completada | [Informe de Fase 3](fase-3-lexer.md), tipos de fuente/spans, lexer y 13 pruebas específicas | El parser/AST pertenece a Fase 4 |
| 4 | Completada (subconjunto inicial) | [Informe de Fase 4](fase-4-parser.md), AST estructural, parser, validación sintáctica de fragmentos con Syn y pruebas de parser/pipeline | No genera Rust ni compila con Cargo; la expansión de sintaxis queda para fases posteriores |
| Portal documental | En curso (base creada) | [website/README.md](../website/README.md), Astro Starlight, páginas por audiencia, referencia generada desde `docs/`; build estático local verificado | No conectado ni publicado aún en la cuenta de Cloudflare Pages; falta decidir dominio |
| 5–14 | Pendiente | Añadir evidencia al cerrar cada fase | Registrar solo bloqueos concretos |

Estados permitidos: **pendiente**, **en curso**, **bloqueada**, **completada**. No uses porcentajes sin una medición reproducible. Al cerrar una fase, registra commit, comandos ejecutados, resultados, criterios cumplidos y limitaciones.
