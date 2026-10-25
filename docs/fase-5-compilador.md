# Fase 5 — Primer compilador funcional

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
- Importación restringida a funciones libres admitidas. Se rechazan items y modificadores que no pueden volver de forma reversible. Los comentarios ordinarios y el formato original no se conservan.

## Límites pendientes

La sintaxis de items y bloques sigue limitada al alcance inicial de Fases 3 y 4. No se admiten aquí dependencias de usuario, proyectos Cargo arbitrarios, importación general de Rust ni la compatibilidad amplia de la matriz. El flujo no sandboxea código de dependencias o macros: la Fase 5 genera un proyecto controlado sin dependencias de usuario y ejecuta código solo ante el comando explícito `ejecutar`. Las políticas de confianza para LSP/playground y la ampliación segura de compilación pertenecen a sus fases correspondientes.

La verificación local de esta fase se realizó con `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`. La CI remota de este repositorio privado está bloqueada por el límite de facturación; por eso esta evidencia local no se declara como CI remota reproducible.

## Decisiones

La generación usa tokens Rust y no concatenación de texto; Cargo se invoca mediante `Command` con argv separados. Los detalles arquitectónicos se registran en [ADR 0002](adr/0002-generacion-y-proceso-cargo.md), y la matriz de cobertura vigente está en [compatibilidad](compatibility.md).
