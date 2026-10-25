# Pipeline del compilador

## Fases

1. **Descubrimiento:** la CLI busca `oxido.toml` declarativo desde la fuente hasta la raíz Git, canonicaliza y resuelve el modo/edición sin ejecutar comandos de configuración.
2. **Fuente:** leer `.ox` con límite de tamaño y conservar bytes, líneas y offsets UTF-8.
3. **Lexer:** tokens Rust base, keywords alias, raw identifiers, comentarios/literales y grupos balanceados; detectar controles bidi.
4. **Modo:** resolver configuración de proyecto y cabecera; rechazar falta o conflicto antes del cuerpo.
5. **Normalización estructural:** convertir bloques indentation a grupos de bloque Rust, añadir terminadores definidos y normalizar keywords fuera de regiones opacas.
6. **Validación sintáctica:** generar token stream/AST; Syn `parse_file` valida el Rust resultante con `full` + `parsing`.
7. **Backend:** emitir Rust estándar con `proc_macro2`/`quote`; formatear estructuralmente con `prettyplease`. No interpolar input como texto.
8. **Mapa:** asociar spans de fragmentos fuente y generado; conservar rangos de item para tokens sintéticos.
9. **Cargo:** la rebanada inicial crea un proyecto Cargo aislado sin dependencias y ejecuta `check` o `run` con `Command`, argv separados, `--offline`, `--locked` y `--message-format=json`.
10. **Diagnósticos:** parsear JSON, convertir spans generados a `.ox`, conservar errores externos no mapeables.
11. **Resultado:** devolver estado/código de salida real. No ejecutar binario salvo comando explícito `ejecutar` o `probar`.

## Fuente de verdad por tipo de error

- Óxido detecta modo, alias, indentación, delimitadores, estructura propia y límites.
- Syn detecta sintaxis Rust inválida en el archivo/token stream transformado.
- `rustc` detecta resolución, tipos, traits, ownership, borrowing, macros/expansión y errores semánticos.
- Cargo resuelve dependencias, perfiles, builds y procesos del proyecto.

El parser Syn no es borrow checker ni type checker. `build.rs` y macros procedurales ejecutan código durante compilación; no se compila un workspace no confiable automáticamente.

## Importar/exportar

Exportar emite el `.rs` generado y un mapa lateral sin rutas absolutas. Importar acepta funciones libres que parsean y regeneran el mismo Rust formateado; rechaza otros items y modificadores fuera del subconjunto. Preserva atributos y documentación Rust como atributos `doc`, también en el ámbito de crate; los comentarios ordinarios y el formato de origen no se conservan. La prueba de ida y vuelta comprueba la documentación conservada y la pérdida de comentarios ordinarios.
