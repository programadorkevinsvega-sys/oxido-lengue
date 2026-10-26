# Roadmap de Óxido

Este roadmap desglosa las fases 0–14 de `plan` en unidades implementables. Es un plan de trabajo, no un informe de capacidades existentes.

## Estado inicial

**Fases 0–8: completadas en el alcance documentado. Fase 9: en curso. Fases 10–14: pendientes.** La Fase 5 entrega una rebanada inicial ejecutable, no compatibilidad general con Rust. Las fases 6–8 amplían sintaxis e interoperabilidad en los subconjuntos indicados por sus matrices. La Fase 9 ya tiene regresiones para comandos, importación de árboles, diagnósticos JSON y formato de ambos modos; el cruce de producciones estructurales del formateador está completado en `testing.md`, y queda validar estos cambios con la CI remota.

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
6. **Macros y atributos:** traducir nombres de macros estándar allowlisted; recorrer aliases en argumentos que son expresiones Rust, y mantener opacos los argumentos de macros externas/desconocidas y macros de metaprogramación. Los atributos solo se traducen con reglas allowlisted.
7. **Compatibilidad:** crear matriz de construcciones conservadas, transformadas, rechazadas y pendientes.
8. **Diagnósticos y posiciones:** definir códigos, severidad, rangos, etiquetas y contrato del mapa `.ox` ↔ `.rs`.
9. **Backend:** especificar generación con árboles de tokens/AST (`proc_macro2`/`quote`), formato intermedio (`prettyplease`) y formato final `rustfmt` fijado, nunca por concatenación de texto no confiable.
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

## Fase 3. Fuentes y lexer inicial

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

## Fase 4. Parser estructural inicial y AST

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

**Criterio de salida:** el corpus válido/inválido cubre AST, spans, recuperación, passthrough con atributos/visibilidad, macros, controles, nombres reservados y semicolon/tail en ambos modos; se verifican diagnósticos Syn y límites de recursión. Cumplido para el subconjunto inicial documentado en [fase-4-parser.md](fase-4-parser.md); las formas fuera de ese subconjunto permanecen parciales o pendientes en compatibilidad.

**Dependencia:** Fase 3.

---

## Fase 5. Primer compilador funcional

**Objetivo:** completar primero una rebanada vertical mínima: exportar, mapear, compilar y ejecutar un programa `.ox` «Hola, mundo». Después se amplía el subconjunto y se implementa la importación reversible.

### Tareas incrementales

1. **Emisor mínimo:** completada para el subconjunto de Hola, mundo, con `quote`/`proc_macro2`, formateo `prettyplease` y validación Syn.
2. **Mapa de posiciones:** completada para spans de fragmentos e items, con serialización lateral y nombres de archivo sin rutas absolutas.
3. **Exportar:** completada; produce Rust inspeccionable y mapa lateral para el subconjunto admitido.
4. **Adaptador Cargo:** completada para el proyecto generado aislado, con `Command`, argumentos separados, límite de tiempo, modo offline y mensajes JSON.
5. **Diagnósticos JSON:** completada para errores de tipos y cabeceras de control; conserva diagnósticos externos que no se pueden mapear.
6. **Compilar y ejecutar:** completada; una prueba compila y ejecuta el ejemplo, compara la salida y comprueba propagación de errores.
7. **Importar y vuelta redonda:** completada para funciones libres admitidas; la prueba CLI verifica `.ox → .rs → .ox → .rs` normalizado y rechazos fuera del subconjunto.

**Entregables:** programa `.ox` real compilado y ejecutado, Rust generado inspeccionable, diagnósticos mapeados y prueba de ida y vuelta del subconjunto reversible.

**Criterio de salida:** cumplido para el alcance inicial descrito en [informe de Fase 5](fase-5-compilador.md). Ejecución automatizada local compara el resultado y no reporta éxito si falla Cargo o `rustc`. La CI remota pública ejecuta las verificaciones del repositorio.

**Dependencia:** Fase 4.

---

## Fase 6. Sintaxis de bloques y bilingüismo

**Objetivo:** completar ambos modos y normalizar alias sin modificar tokens protegidos.

### Subfases

1. **Modo indentado:** completado para funciones, controles anidados, continuaciones y terminadores.
2. **Modo de llaves:** completado con la misma AST, semántica de tail y formas de control admitidas.
3. **Configuración:** completada; se admite modo de proyecto sin cabecera y se rechaza una cabecera inicial que lo contradiga.
4. **Alias:** normalización tokenizada completada en firmas, visibilidad, tipos, patrones, expresiones y items Rust passthrough fuera de regiones opacas.
5. **Escape:** comprobados `r#...` y diagnósticos `OX3001` con sugerencia en nombres de funciones, parámetros, patrones anidados, bindings y patrones de `para`.
6. **Macros:** comprobado que las invocaciones y atributos conservan nombre y tokens internos, incluidos grupos anidados.
7. **Equivalencia:** completada con pruebas que compilan y ejecutan el mismo caso en ambos modos y comparan su salida.
8. **Formato:** la salida Rust de ambos modos se valida con Syn, usa prettyplease como formato intermedio y termina en rustfmt fijado; el formateo no reescribe ni cambia silenciosamente el archivo fuente `.ox`.

**Entregables:** completados para el subconjunto inicial de funciones/control descrito en [informe de Fase 6](fase-6-sintaxis-bilinguismo.md); matriz de sintaxis y bilingüismo con pruebas de equivalencia y regresión.

**Criterio de salida:** cumplido para el subconjunto indicado; pruebas verifican que los modos no mezclan delimitadores, conservan la regla `;`/tail y compilan con comportamiento equivalente. Items con cuerpos indentados y nuevas formas de lenguaje quedan delimitados como Fase 7.

**Dependencia:** Fase 4; el emisor inicial de Fase 5 permitió verificar ambos modos end-to-end.

---

## Fase 7. Lenguaje inicial

**Objetivo:** ampliar el subconjunto mediante capacidades con criterios independientes. El alcance inicial de Fase 7 está cerrado; las extensiones posteriores se registran como cambios acotados sin reabrir criterios ya aceptados.

### Subfases

1. **Variables y expresiones:** completadas para `let`/`dejar`, `dejar mutable`, alias `&mutable`, literales, llamadas y expresiones Rust; pruebas positivas/negativas y de tipo.
2. **Funciones:** completadas para `funcion`, `principal`→`main`, parámetros, tipos, retorno, visibilidad y semicolon/tail.
3. **Control:** condiciones y bucles iniciales en ambos modos, con equivalencia ejecutable verificada en Fase 6.
4. **Tipos compuestos:** tuplas, arrays y primitivos preservados como Rust y probados en ejecución.
5. **Items:** campos indentados de `struct`, variantes indentadas de `enum`; `impl` y métodos en passthrough Rust con llaves.
6. **Referencias:** referencias y borrowing delegados a Rust, con prueba end-to-end.
7. **Módulos e imports:** `use` indentado; módulos/imports Rust en modo llaves.
8. **Errores y tipos comunes:** `Result`, `Option` y `?` probados mediante compilación y ejecución.
9. **Aliases de prelude:** completada para tipos, variantes, primitivos, traits, métodos y rutas `std` enumerados en [aliases](aliases.md).
10. **Macros/atributos:** completada para aliases estándar y atributos allowlisted; los nombres Rust originales siguen válidos, se recorren aliases anidados en macros estándar expresivas y las DSL externas permanecen opacas. La prueba E2E de Fase 8 compila y ejecuta todas las aliases estándar ejecutables mediante Cargo/rustc; `compile_error!` tiene caso negativo.
11. **Entrada principal y mutabilidad:** completada para `principal`→`main`, `dejar`/`dejar mutable` y `mutable` en referencias/parámetros, más colisión y escape `r#`.
12. **Errores y diagnóstico:** completada para el inventario de códigos rustc común documentado; conserva códigos/spans JSON y no depende de salida humana.
13. **Pruebas diferenciales:** completada con corpus positivo/negativo, ejecución end-to-end, aliases y matriz reproducibles.

**Entregables:** completados; informe, matriz y ejemplos ejecutables cubren las subfases. Formato, pruebas de workspace, Clippy y build de documentación verdes.

**Criterio de salida:** cumplido para el alcance inicial; las familias de aliases tienen pruebas de conversión y ejemplos end-to-end para el subconjunto indicado. Los argumentos de macros estándar expresivas se normalizan selectivamente; las DSL externas permanecen opacas. Errores comunes se mapean desde JSON y no se declara compatibilidad general. La sintaxis indentada ampliada se implementa en Fase 8.

**Dependencia:** Fase 6; algunas características pueden desarrollarse en paralelo si no comparten gramática/backend.

---

## Fase 8. Integración con Rust

**Objetivo:** ampliar la sintaxis estructural indentada y la interoperabilidad con Rust, preservando una sola semántica Rust.

### Subfases

1. **Estructura indentada:** completada para el alcance inicial con `impl` y métodos, `trait` con firmas/defaults, módulos anidados y `coincidir` con brazos de expresión o bloque. La cobertura incluye receivers y funciones asociadas, genéricos/`donde`, varios bloques por tipo, visibilidad/atributos/docs, items asociados, bloque vacío e indentación inválida, regla del `;` final, equivalencia con llaves y mapeo de errores rustc a `.ox`.
2. Dependencias Cargo reales y configuración reproducible. **Verificado para crates local y registry:** SemVer validado con `semver`; crates.io por defecto, `path` requiere `--permitir-rutas-locales`, git/URL rechazados. La vista previa lista paquetes y diff; `--confirmar` hace solo `cargo fetch`. Compilar/ejecutar exige `--locked --offline` y `--confiar-dependencias`; OX7001 explica el lock faltante. E2E verifica checksums, lock estable, vendor alterado, build.rs/proc-macro bajo confianza y Rust determinista en diferentes directorios.
3. Genéricos, traits y lifetimes dentro del alcance del frontend. **Verificado para el corpus actual:** firmas genéricas con lifetime, `impl Fn`, bounds `where`, trait e implementación genéricos y tipos asociados pasan por generación y Cargo.
4. Pattern matching, closures e iteradores, completando patrones y expresiones Rust preservados por el frontend. **Verificado para el corpus actual:** closure como argumento, `Iterator::find`, patrones `Some`/`None` y brazos indentados/llaves compilan y producen el mismo resultado.
5. Macros declarativas y atributos, preservando árboles de tokens. **Verificado para casos concretos:** `macro_rules!`, Serde derive/atributo, `tokio::main`, `tokio::select!`, `serde_json::json!` y `paste!` con versiones fijadas; no implementa expansor Óxido.
6. `async`/`await` y concurrencia. **Verificado para corpus concreto:** `.await` con `futures-executor`, `tokio::main`/`select!` y `std::thread`; el runtime lo elige la aplicación.
7. Bibliotecas externas de referencia y pruebas de conformidad. **Corpus registrado:** `itoa`, `futures-executor`, Serde, Tokio, `serde_json`, `paste` y crates locales. No implica compatibilidad con crates arbitrarios.
8. `unsafe`: preservar semántica Rust y ofrecer auditoría opcional. **Verificado para caso concreto:** una API de punteros válida compila y ejecuta sin rechazo por defecto; la auditoría puede rechazarla bajo opción explícita. Workspace Trust/sandbox de compilaciones editoriales corresponde a Fase 11.
9. **Matriz por versión/edición:** completada para Rust 2024, MSRV 1.99.1 y el corpus probado; límites por característica y casos no evaluados constan en [compatibility.md](compatibility.md).
10. **Diagnósticos y preservación cruzada:** verificados para funciones, métodos, módulos y la macro externa probada. Cargo sobre el árbol exportado informa ubicaciones `.rs`; los mapas laterales no se consumen fuera de Óxido.

**Entregables:** suites de conformidad, proyectos de referencia versionados y gramática indentada validada con Cargo/rustc.

**Criterio de salida:** cumplido para las construcciones, versiones y regresiones registradas en [Fase 8](fase-8-integracion-rust.md). El objetivo es interoperabilidad sin cambiar fuentes de bibliotecas, no compatibilidad universal. Workspace Trust corresponde a la integración del editor en Fase 11. La CLI no sandboxa `build.rs` ni macros procedurales; el operador debe confiar el proyecto antes de resolver o compilar esas dependencias.

**Dependencia:** Fase 7.

---

## Fase 9. CLI y experiencia de proyecto

**Objetivo:** entregar comandos estables y seguros para tareas habituales.

**Estado actual:** en curso. Las pruebas CLI verifican que `nuevo` cree un proyecto ejecutable, que `probar` ejecute una prueba real y propague fallos, que `formato` sea idempotente y conserve comentarios en ambos modos, y que el formato indentado reindente bloques/continuaciones, normalice espaciado, envuelva listas largas y mantenga comentarios, literales y grupos opacos. La importación de árboles cubre `.rs`/`.ox` mixtos, reescritura de `#[path]` relativo y rechazos de rutas inseguras. `compilar`, `ejecutar` y `probar` ofrecen `--json` con diagnósticos y ubicaciones relativas cuando están disponibles. La matriz CLI fija la salida `.ox`, exige que Rust exportado antes/después coincida carácter por carácter con `rustfmt` 1.99.1, compila ambos con `rustc` y ejecuta un fixture. El cruce de todas las producciones estructurales iniciales está documentado en [testing.md](testing.md); no afirma equivalencia universal para cada expresión Rust delegada a Syn.

### Subfases

1. Definir nombres, argumentos, ayuda y códigos de salida.
2. Crear proyectos con plantillas mínimas verificables. La plantilla actual de `nuevo` tiene una prueba CLI que la compila y ejecuta; se comprueba también el rechazo de un directorio ya existente sin modificarlo.
3. **Compilación y ejecución:** `compilar` y `ejecutar` están disponibles. Las pruebas CLI validan `nuevo` de extremo a extremo y que `probar` ejecute un ítem `#[test]`, propague el resultado fallido y rechace `--release`. Sin ítems `#[test]`, Cargo informa cero pruebas. Un comando `verificar` separado no existe.
4. **Formato:** `oxido formato <archivo.ox> [--comprobar]` tiene regresiones CLI de idempotencia y conservación de comentarios para ambos modos. En modo indentado normaliza sangría a cuatro espacios, espaciado de tokens en código no opaco y envuelve listas largas en `()`/`[]` a 100 columnas; conserva literales, comentarios y DSLs opacas, y valida el resultado con el compilador. `formatter_conformance_matrix_matches_rustfmt_after_rust_export` fija el formato esperado de cinco grupos (items/tipos; traits, impl y módulos; macros/comentarios/async; expresiones/patrones; controles y bloques), comprueba idempotencia y exige que el Rust exportado, antes y después, coincida carácter por carácter con `rustfmt` 1.99.1. El cruce de producciones estructurales de `grammar.md` está completo y documentado en `testing.md`; las expresiones, tipos, patrones y DSLs que Rust delega a Syn no se enumeran exhaustivamente. La sintaxis `.ox` sigue convenciones de layout propias de Óxido; la igualdad carácter por carácter exigida es para el Rust generado.
5. **Diagnósticos CLI:** `compilar`, `ejecutar` y `probar` aceptan `--json`. El esquema versión 1 informa éxito, diagnósticos, código/severidad/mensaje, archivo relativo y posición si existen, y salida del programa saneada. Errores tempranos usan `OX0001`; fallos Cargo sin diagnóstico mapeable usan `OX5001`. La regresión comprueba éxito y error de tipo con ubicación `.ox`, salida JSON por stdout y ausencia de rutas absolutas.
6. **Importación/exportación:** pruebas CLI importan y vuelven a exportar árboles convencionales con `.rs`, `.ox` y `#[path]` relativo, conservan comentarios y ejecutan el resultado Rust. Se rechazan destinos existentes, enlaces simbólicos de destino, rutas fuera del proyecto, rutas ausentes y colisiones antes de escribir.
7. Implementar informe de superficie de riesgo (`unsafe`, `build.rs`, macros procedurales y llamadas externas detectables); ya hay una prueba CLI que verifica el informe y que no expone rutas absolutas.
8. Endurecer rutas, enlaces simbólicos, configuración y ejecución de procesos; las regresiones actuales cubren las rutas de importación y escritura descritas arriba. El formato opera sobre el archivo solicitado y no sigue enlaces de salida ni crea archivos auxiliares.

**Entregables:** CLI documentada y pruebas de integración de comandos.

**Criterio de salida:** la revisión de cobertura estructural está completada y cruzada con fixtures en `testing.md`. Para los fixtures de la matriz, el Rust exportado debe coincidir carácter por carácter con `rustfmt`; el `.ox` indentado sigue convenciones de layout propias de Óxido y no tiene que ser texto Rust idéntico. La matriz compila los exports y ejecuta un caso; no pretende enumerar todas las expresiones Rust que Syn admite. La fase queda pendiente hasta ejecutar correctamente la suite remota sobre el commit con esta revisión. Los demás comandos y escenarios enumerados tienen pruebas CLI positivas/negativas, resultados verificables, destinos de importación restringidos y roundtrip ejecutable de árboles mixtos con `#[path]`.

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
7. Mantener proyectos de referencia y harnesses reutilizables para las fases que los necesiten. La aprobación de integración con crates, macros, runtimes o `unsafe` pertenece a la fase funcional correspondiente (Fase 8 para los criterios de ADR 0007), no se traslada a este laboratorio.
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
5. Integración con `rust-analyzer` para tipos inferidos, hover, inlay hints y autocompletado sobre Rust generado; mapear todas las posiciones al `.ox` y no inventar tipos desde el LSP.
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

| Fase | Estado | Evidencia y comando de verificación | Bloqueos |
|---|---|---|---|
| 0 | Completada | [Informe de auditoría](auditoria-fase-0.md); revisión documental histórica (no existía suite ejecutable) | Ninguno para iniciar Fase 1 |
| 1 | Completada (decisiones normativas) | [Especificación](language-spec.md), [gramática](grammar.md), [aliases](aliases.md); `cargo test -p oxido-language --locked` valida fixtures de lexer/parser | Sin bloqueos dentro de las decisiones 0.1 |
| 2 | Completada (infraestructura inicial) | [Informe de Fase 2](fase-2-infraestructura.md); `cargo test --workspace --locked` | Ninguno registrado |
| 3 | Completada (fuentes y lexer inicial) | [Informe de Fase 3](fase-3-lexer.md); `cargo test -p oxido-language --test lexer --locked` | No implica parser ni generación |
| 4 | Completada (parser estructural inicial) | [Informe de Fase 4](fase-4-parser.md); `cargo test -p oxido-language --test parser --locked` | No genera Rust por sí sola |
| 5 | Completada (compilador inicial) | [Informe de Fase 5](fase-5-compilador.md); `cargo test -p oxido-cli --test cli --locked` comprueba exportar, compilar, ejecutar y roundtrip | Sin bloqueos dentro del subconjunto reversible y ejecutable |
| 6 | Completada (bloques y aliases iniciales) | [Informe de Fase 6](fase-6-sintaxis-bilinguismo.md); `cargo test -p oxido-language -p oxido-rust-backend --locked` | La ampliación de items estructurales se documenta en Fases 7–8 |
| 7 | Completada (alcance inicial del lenguaje) | [Informe de Fase 7](fase-7-lenguaje-inicial.md); `cargo test -p oxido-compiler --test pipeline --locked` | Alcance ampliado en Fase 8 |
| Portal documental | Publicado en Cloudflare Pages | [website/README.md](../website/README.md), Astro Starlight; build y publicación estática operativos | Sitemap requiere configurar el dominio `site`; Starlight muestra un aviso informativo al resolver la ruta 404 |
| 8 | Completada (corpus de interoperabilidad verificado) | [Informe de Fase 8](fase-8-integracion-rust.md); `cargo test --workspace --locked` y regresiones E2E de macros/interoperabilidad | No implica compatibilidad universal; Workspace Trust/sandbox permanecen en Fase 11 |
| 9 | En curso | `nuevo`, `formato`, `probar`, `importar-proyecto`; `--json` en `compilar`/`ejecutar`/`probar` | Completar pruebas de conformidad del formato indentado frente al estilo Rust; ver Fase 9 |
| 10–14 | Pendiente | Añadir evidencia al cerrar cada fase | Registrar solo bloqueos concretos |

Estados permitidos: **pendiente**, **en curso**, **bloqueada**, **completada**. No uses porcentajes sin una medición reproducible. Al cerrar una fase, registra commit, comandos ejecutados, resultados, criterios cumplidos y limitaciones.

Las fases 0–8 están completadas para los alcances declarados. La Fase 9 está en curso. Las limitaciones de la matriz no se consideran características implementadas.
