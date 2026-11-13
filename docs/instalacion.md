# Instalación y primeros pasos

Óxido se distribuye hoy como código fuente en este repositorio. No hay paquetes publicados, instaladores firmados ni binarios en un marketplace. Las instrucciones siguientes se verificaron en Linux con Rust 1.99.1; la CLI también se prueba en Windows y macOS en CI.

## Requisitos

- Rust 1.99.1 o posterior y Cargo. La edición objetivo del workspace es Rust 2024.
- `rustfmt` del mismo toolchain. Sin él, la generación devuelve `OX4001`.
- Opcional para el editor: `rust-analyzer` y la extensión de VS Code. Consulta la [guía del editor](vscode.md).

Comprueba el toolchain:

```sh
rustc --version
cargo --version
rustfmt --version
```

## Compilar desde el código fuente

Desde la raíz del repositorio:

```sh
cargo build --release --locked -p oxido-cli
./target/release/oxido --help
```

El binario se llama `oxido`. Para instalarlo en el `PATH` del usuario:

```sh
cargo install --path crates/oxido-cli --locked
oxido --help
```

`cargo install` compila en modo release y no publica nada. El crate tiene `publish = false`.

## Crear y ejecutar un proyecto

```sh
oxido nuevo demo
oxido ejecutar demo/src/main.ox
```

`oxido nuevo` crea `oxido.toml` y `src/main.ox` con un programa mínimo. La salida esperada es `Hola, Óxido.`.

También puedes ejecutar los ejemplos del repositorio:

```sh
oxido ejecutar examples/hola_mundo.ox
oxido ejecutar examples/estructuras.ox
oxido ejecutar examples/interoperabilidad.ox
```

## Comandos disponibles

| Comando | Función | Límite actual |
|---|---|---|
| `nuevo <dir>` | Crea un proyecto mínimo | Plantilla fija en modo indentado |
| `formato <archivo.ox> [--comprobar]` | Formatea y comprueba idempotencia | Solo sintaxis cubierta por la matriz |
| `exportar <archivo.ox> --salida <archivo.rs>` | Genera Rust, mapa de origen y resuelve módulos | El Rust final pasa por `rustfmt` |
| `importar <archivo.rs> --salida <archivo.ox> [--espanol]` | Importa un archivo Rust | Por defecto conserva el código tal cual; `--espanol` traduce el subconjunto de funciones libres |
| `importar-proyecto <dir> --salida <dir> [--espanol]` | Recorre un árbol Rust | Importa cada `.rs` tal cual; `--espanol` traduce el subconjunto reversible. Los originales quedan en `.oxido/original/` |
| `analizar-rust <archivo.rs>` | Valida sintaxis Rust | No compila |
| `check <archivo.ox>` (`verificar`) | Valida con `cargo check`, sin dejar un ejecutable | Requiere `rustfmt` y `Cargo.lock` si hay dependencias |
| `build <archivo.ox>` (`compilar`) | Genera un binario persistente | `--release` genera el perfil optimizado; salida en `target/oxido/{debug,release}` |
| `run <archivo.ox>` (`ejecutar`) | Compila y ejecuta el programa | `--release` selecciona el perfil optimizado |
| `test <archivo.ox>` (`probar`) | Ejecuta `cargo test` | Un archivo sin `#[test]` puede dar cero pruebas |
| `auditar-unsafe <archivo.ox> [--rechazar]` | Busca `inseguro` | Análisis léxico, no semántico |
| `auditar-proyecto <dir>` | Inventario estático de riesgo | No sigue symlinks ni ejecuta Cargo |
| `descargar-dependencias <archivo.ox>` | Muestra el plan de dependencias | Solo descarga con `--confirmar` |
| `aliases` | Lista alias del prelude | No amplía la matriz de compatibilidad |

La [matriz de compatibilidad](compatibility.md) delimita la sintaxis que puede usarse. `check`/`verificar` valida sin producir un ejecutable; `build`/`compilar` genera artefactos bajo `target/oxido/{debug,release}`; `run`/`ejecutar` además inicia el programa. Añade `--release` a `build` para el perfil optimizado de Cargo. La CLI no sandboxa código de dependencias: las acciones que compilan con dependencias exigen `--confiar-dependencias` y pueden ejecutar `build.rs` y macros procedurales.

## Dependencias

Declara dependencias en `oxido.toml`:

```toml
[language]
syntax = "indentation"
edition = "2024"

[dependencies.itoa]
version = "1.0"
```

Revisa el plan con `oxido descargar-dependencias archivo.ox`, confírmalo con `--confirmar` y compila con `--confiar-dependencias`. La compilación usa `--locked --offline` y falla con `OX7001` si falta el lockfile. Las dependencias `path` requieren `--permitir-rutas-locales` en la descarga; git y URL no están soportadas.

## Editor

Compila `oxido-lsp`, empaqueta el VSIX e instálalo. Los pasos, comandos y límites están en la [guía de VS Code y LSP](vscode.md).

## Solución de problemas

| Síntoma | Causa probable | Acción |
|---|---|---|
| `OX4001` al generar | Falta `rustfmt` o el código generado no es válido | Instala el componente `rustfmt` del toolchain y revisa el diagnóstico |
| `OX7001` | Falta `Cargo.lock` con dependencias declaradas | Ejecuta `oxido descargar-dependencias ... --confirmar` |
| `no se pudo determinar el modo de sintaxis` | El archivo no tiene cabecera ni `oxido.toml` | Añade `sintaxis indentacion` o `sintaxis llaves` |
| `oxido-lsp` no aparece en VS Code | El binario no está en `PATH` | Configura `oxido.serverPath` con la ruta absoluta |
| Sin hover, completado ni renombrado de Rust | `rust-analyzer` no está disponible o el workspace no es confiable | Instala el componente y concede Workspace Trust |
| La compilación con dependencias falla por red | Se usa `--locked --offline` | Descarga antes con `--confirmar` en un entorno revisado |

Los códigos y mensajes están en la [referencia de diagnósticos](diagnostics.md). Si el problema no está cubierto, abre un issue con el comando ejecutado, la versión de Rust y el diagnóstico completo, sin incluir rutas privadas ni secretos.

## Desinstalar

Si instalaste con Cargo:

```sh
cargo uninstall oxido-cli
```

Si usaste el binario de `target/`, elimínalo junto con el directorio del proyecto. Los proyectos generados no escriben fuera de su directorio salvo la caché del editor, que se guarda fuera del workspace y se describe en la [guía del editor](vscode.md).
