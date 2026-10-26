# Fase 5 — Primer compilador funcional (alcance original)

**Estado:** completada para la rebanada vertical inicial. Esto no representa compatibilidad general con Rust ni un compilador listo para producción.

## Resultado verificable

`examples/hola_mundo.ox` recorre el flujo real `.ox → lexer/parser inicial → tokens Rust → Syn → archivo .rs → Cargo/rustc → ejecución`. La prueba de integración comprueba la salida exacta `Hola, mundo.`. La CLI también exporta e importa el subconjunto reversible de funciones libres y comprueba una vuelta `.ox → .rs → .ox → .rs` comparando el Rust normalizado.

## Alcance entregado

- Generación de tokens con `proc_macro2`/`quote`; validación con Syn y formateo con `prettyplease`.
- Mapa lateral serializable entre spans de fuente y Rust generado. Los nombres incluidos son basenames, no rutas absolutas.
- Adaptador Cargo con argumentos separados, sin shell, límite de tiempo, ejecución offline y lectura de eventos JSON.
- Mapeo de diagnósticos `rustc` a expresiones y cabeceras de control `.ox`; errores sin ubicación mapeable se conservan como externos.
- Comandos CLI `exportar`, `importar`, `compilar` y `ejecutar`. Solo `ejecutar` corre el binario generado.
- Configuración `oxido.toml` de datos declarativos para resolver el modo del archivo.
- Importación restringida al subconjunto reversible de funciones libres y passthrough de items Rust, suficiente para recorrer árboles con `mod` y archivos mixtos. Se rechazan modificadores de función que no pueden volver de forma reversible. La exportación conserva comentarios ordinarios no opacos insertándolos desde los spans del lexer tras el formateo de Rust; la importación conserva el texto de origen y cambia únicamente `fn` por `funcion`. La prueba CLI inspecciona ambos archivos y confirma que los comentarios sobreviven a `.ox → .rs → .ox → .rs`. El formateo puede normalizar espacios y estructura Rust; no promete conservar el formato original. Los comentarios ordinarios dentro de argumentos opacos de macros/atributos se rechazan explícitamente para evitar perderlos o alterar DSLs.

## Límites de la entrega original

En el momento de esta entrega, la sintaxis de items y bloques seguía limitada al alcance inicial de Fases 3 y 4, sin dependencias de usuario ni importación de árboles. Esos límites se ampliaron después y su estado vigente está en [compatibility.md](compatibility.md), [Fase 8](fase-8-integracion-rust.md) y el [roadmap](roadmap.md). La CLI actual sigue sin sandbox para código de dependencias; `--confiar-dependencias` solo registra la autorización explícita del operador.

Este inventario registra los comandos y el alcance originales de Fase 5. La CLI actual añade `nuevo`, `formato`, `probar` e `importar-proyecto`; las capacidades y límites vigentes del formato constan en [compatibility.md](compatibility.md), y los criterios pendientes de CLI están en [Fase 9](roadmap.md).

La verificación original incluyó `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`. La CI remota del repositorio público ejecuta los checks de su workflow; esta mención no representa el resultado de una ejecución actual.

## Decisiones

La generación usa tokens Rust y no concatenación de texto; Cargo se invoca mediante `Command` con argv separados. Los detalles arquitectónicos se registran en [ADR 0002](adr/0002-generacion-y-proceso-cargo.md), y la matriz de cobertura vigente está en [compatibilidad](compatibility.md).
