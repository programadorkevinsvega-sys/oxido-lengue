# Pipeline del compilador

## Fases

1. **Descubrimiento:** canonicalizar el workspace, localizar `oxido.toml` y resolver un único modo/edición sin ejecutar comandos de configuración.
2. **Fuente:** leer `.ox` con límite de tamaño y conservar bytes, líneas y offsets UTF-8.
3. **Lexer:** tokens Rust base, keywords alias, raw identifiers, comentarios/literales y grupos balanceados; detectar controles bidi.
4. **Modo:** resolver configuración de proyecto y cabecera; rechazar falta o conflicto antes del cuerpo.
5. **Normalización estructural:** convertir bloques indentation a grupos de bloque Rust, añadir terminadores definidos y normalizar keywords fuera de regiones opacas.
6. **Validación sintáctica:** generar token stream/AST; Syn `parse_file` valida el Rust resultante con `full` + `parsing`.
7. **Backend:** emitir Rust estándar con `proc_macro2`/`quote`; formatear estructuralmente con `prettyplease`. No interpolar input como texto.
8. **Mapa:** asociar spans fuente y generado, incluidos tokens sintéticos.
9. **Cargo:** invocar `cargo check/build/test/run` con `Command` y argv separados, `--message-format=json`, `--locked` cuando exista lockfile, cwd/rutas dentro del workspace permitido.
10. **Diagnósticos:** parsear JSON, convertir spans generados a `.ox`, conservar errores externos no mapeables.
11. **Resultado:** devolver estado/código de salida real. No ejecutar binario salvo comando explícito `ejecutar` o `probar`.

## Fuente de verdad por tipo de error

- Óxido detecta modo, alias, indentación, delimitadores, estructura propia y límites.
- Syn detecta sintaxis Rust inválida en el archivo/token stream transformado.
- `rustc` detecta resolución, tipos, traits, ownership, borrowing, macros/expansión y errores semánticos.
- Cargo resuelve dependencias, perfiles, builds y procesos del proyecto.

El parser Syn no es borrow checker ni type checker. `build.rs` y macros procedurales ejecutan código durante compilación; no se compila un workspace no confiable automáticamente.

## Importar/exportar

Exportar emite el `.rs` generado y un mapa lateral de posiciones. Importar Rust solo acepta el subconjunto reversible de `compatibility.md`; no borra macros, comentarios, atributos ni sintaxis desconocida intentando traducirla. Debe fallar con diagnóstico enumerando la construcción fuera del subconjunto.
