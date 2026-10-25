# Módulos Óxido en una jerarquía de archivos

Este proyecto comprueba `modulo` entre archivos y directorios. La fuente está en `fuente/`; `rust/` contiene el árbol Rust generado. La estructura corresponde a:

```text
main.ox
├── aplicacion.ox
│   └── aplicacion/servicios.ox
└── dominio.ox
    └── dominio/modelos.ox
```

`rust/` conserva el árbol generado archivo por archivo y se compila como proyecto Cargo independiente:

```sh
CARGO_TARGET_DIR=/tmp/modulos-jerarquicos-target cargo run --locked --offline --manifest-path prueba/modulos_jerarquicos/rust/Cargo.toml --release
```

## Resolución y exportación del proyecto completo

También se puede compilar directamente desde la raíz `.ox`; el compilador sigue las declaraciones `modulo`, resuelve cada archivo y subdirectorio y compila un solo binario Cargo temporal:

```sh
cargo run --locked -p oxido-cli -- compilar prueba/modulos_jerarquicos/fuente/main.ox
cargo run --locked -p oxido-cli -- ejecutar prueba/modulos_jerarquicos/fuente/main.ox --release
```

La exportación recursiva genera el árbol Rust completo y un mapa por cada archivo `.ox`:

```sh
cargo run --locked -p oxido-cli -- exportar prueba/modulos_jerarquicos/fuente/main.ox --salida prueba/modulos_jerarquicos/rust-recursivo/src/main.rs
CARGO_TARGET_DIR=/tmp/modulos-jerarquicos-recursivo-target cargo run --locked --offline --manifest-path prueba/modulos_jerarquicos/rust-recursivo/Cargo.toml --release
```

La regresión automatizada compara los `.rs` de `rust/` con la salida recursiva, compila y ejecuta ambos proyectos por separado, y compara su salida con `oxido ejecutar`. La exportación desde la raíz detecta los módulos y escribe el árbol completo de una vez; no hace falta invocar `exportar` por cada submódulo.

También se admiten atributos `#[path = "ruta.ox"]` y `#[path = "ruta.rs"]`. Las rutas deben ser relativas y resolver a archivos regulares dentro del proyecto. Al exportar, Óxido ajusta la ruta para el árbol Rust generado. Las rutas que escapan de la raíz, los enlaces simbólicos como archivo final y las extensiones distintas se rechazan.

`oxido importar` se probó sobre `rust/src/main.rs`, y devuelve `la importación reversible inicial solo admite funciones libres`: el importador actual **no importa declaraciones ni árboles de módulos**. Cada exportación genera también su mapa `.oxido-map.json`.

Verificación realizada el 25-10-2026: `oxido compilar` sobre la raíz terminó correctamente; `oxido ejecutar --release`, la exportación archivo por archivo y la exportación recursiva compilaron/ejecutaron el mismo flujo y mostraron `Ana aprobó: true`. La importación del archivo raíz continúa fuera del alcance documentado del importador.
