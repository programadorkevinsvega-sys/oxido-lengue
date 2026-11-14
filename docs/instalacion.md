# Instalación y primeros pasos

Óxido 0.1.0 está publicado en [GitHub Releases](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0), con paquetes Debian para la CLI y el editor, y VSIX por plataforma. Los paquetes no están firmados; descarga `SHA256SUMS` de la misma Release y verifica cada archivo antes de instalarlo. La extensión todavía no está publicada en Marketplace. Las instrucciones de compilación se verificaron en Linux con Rust 1.99.1; la CLI también se prueba en Windows y macOS en CI.

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
./target/release/oxido --version
```

El binario se llama `oxido`. Para instalarlo en el `PATH` del usuario:

```sh
cargo install --path crates/oxido-cli --locked
oxido --version
```

`cargo install` compila en modo release y no publica nada. El crate tiene `publish = false`.

Comprueba la versión instalada con `oxido --version` (o `oxido -V`). La salida tiene el formato `oxido <versión>` y corresponde a la versión del paquete de la CLI.

## Instalar desde un paquete Debian

Descarga el paquete para tu arquitectura y `SHA256SUMS` de la [Release v0.1.0](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0). En una terminal, desde el directorio donde guardaste ambos archivos, verifica e instala:

- [Óxido CLI para Debian/Ubuntu amd64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido_0.1.0-1_amd64.deb)
- [Óxido CLI para Debian/Ubuntu arm64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido_0.1.0-1_arm64.deb)
- [SHA256SUMS](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/SHA256SUMS)

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./oxido_*_amd64.deb
oxido --version
```

La opción `--ignore-missing` permite verificar el paquete descargado aunque `SHA256SUMS` también enumere la otra arquitectura. Para arm64, cambia el patrón por `./oxido_*_arm64.deb`. Después de instalar el paquete, provisiona Rust en el prefijo aislado de Óxido:

```sh
sudo oxido-instalar-toolchain
```

La instalación de Rust es explícita; `apt` no descarga toolchains desde scripts mantenedores. El Rust del sistema y del usuario no se modifica. `sudo apt remove oxido` elimina la CLI y conserva `/opt/oxido`.

Si guardaste el `.deb` dentro de `~/Descargas` y `apt` avisa que `_apt` no puede acceder al archivo, la instalación puede continuar con una descarga sin aislamiento como root. Es un aviso de permisos del directorio local, no un fallo del paquete. Para evitarlo, copia el paquete a una ruta legible por `_apt` y usa esa copia:

```sh
cp ./oxido_0.1.0-1_amd64.deb /tmp/
sudo apt install /tmp/oxido_0.1.0-1_amd64.deb
```

## Crear y ejecutar un proyecto

```sh
oxido nuevo demo
cd demo
oxido verificar
oxido compilar
oxido ejecutar
```

`oxido nuevo` crea `oxido.toml` y `src/main.ox` con un programa mínimo. Los comandos sin ruta descubren ese `oxido.toml` desde el directorio actual y usan `src/main.ox`. `oxido compilar` genera un proyecto Cargo persistente en `target/oxido/` (incluye `Cargo.toml`, `src/main.rs`, los módulos Rust y sus fuentes `.ox`, como `mod.ox`) y delega la compilación en Cargo. El binario queda bajo `target/oxido/target/debug/`; `oxido ejecutar` compila y lo ejecuta mediante Cargo. La salida esperada es `Hola, Óxido.`. El workflow de paquetes Debian también prueba este ciclo desde el `.deb` instalado y tras instalar la toolchain aislada.

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
| `formato [<archivo.ox|directorio>] [--comprobar]` | Formatea y comprueba idempotencia | Sin ruta o con directorio usa `src/main.ox` |
| `exportar <archivo.ox> --salida <archivo.rs>` | Genera Rust, mapa de origen y resuelve módulos | El Rust final pasa por `rustfmt` |
| `importar <archivo.rs> --salida <archivo.ox> [--espanol]` | Importa un archivo Rust | Por defecto conserva el código tal cual; `--espanol` traduce el subconjunto de funciones libres |
| `importar-proyecto <dir> --salida <dir> [--espanol]` | Recorre un árbol Rust | Importa cada `.rs` tal cual; `--espanol` traduce el subconjunto reversible. Los originales quedan en `.oxido/original/` |
| `analizar-rust <archivo.rs>` | Valida sintaxis Rust | No compila |
| `verificar [<archivo.ox|directorio>]` (`check`) | Valida con `cargo check`, sin dejar un ejecutable | Sin ruta o con directorio usa `src/main.ox` |
| `compilar [<archivo.ox|directorio>]` (`build`) | Genera un binario persistente | `--release` genera el perfil optimizado |
| `ejecutar [<archivo.ox|directorio>]` (`run`) | Compila y ejecuta el programa | `run` es alias de `ejecutar`; `--release` selecciona el perfil optimizado |
| `probar [<archivo.ox|directorio>]` (`test`) | Ejecuta `cargo test` | Un archivo sin `#[test]` puede dar cero pruebas |
| `auditar-unsafe <archivo.ox> [--rechazar]` | Busca `inseguro` | Análisis léxico, no semántico |
| `auditar-proyecto <dir>` | Inventario estático de riesgo | No sigue symlinks ni ejecuta Cargo |
| `descargar-dependencias <archivo.ox>` | Muestra el plan de dependencias | Solo descarga con `--confirmar` |
| `aliases` | Lista alias del prelude | No amplía la matriz de compatibilidad |

La [matriz de compatibilidad](compatibility.md) delimita la sintaxis que puede usarse. Los comandos `formato`, `verificar`, `compilar`, `ejecutar` y `probar` aceptan un archivo `.ox` o un directorio; sin ruta, buscan `oxido.toml` desde el directorio actual hacia arriba y eligen `src/main.ox`. Los mensajes de uso presentan los nombres en español; `run` sigue disponible como alias de `ejecutar`. `verificar` no produce un ejecutable; para proyectos `.ox`, `compilar` genera el proyecto Cargo en `target/oxido/` y sus binarios en `target/oxido/target/{debug,release}`; `ejecutar` además inicia el programa. Añade `--release` a `compilar` o `ejecutar` para el perfil optimizado de Cargo. La CLI no sandboxa código de dependencias: las acciones que compilan con dependencias exigen `--confiar-dependencias` y pueden ejecutar `build.rs` y macros procedurales.

## Dependencias

Declara dependencias en `oxido.toml`:

```toml
[language]
syntax = "indentation"
edition = "2024"

[dependencies.itoa]
version = "1.0"

# Ambas secciones siguientes son opcionales.
[package]
name = "demo"
version = "0.1.0"

[[bin]]
name = "demo"
path = "src/main.ox"
```

Revisa el plan con `oxido descargar-dependencias archivo.ox`, confírmalo con `--confirmar` y compila con `--confiar-dependencias`. La compilación usa `--locked --offline` y falla con `OX7001` si falta el lockfile cuando hay dependencias. Un proyecto sin dependencias no necesita un `Cargo.lock` persistente; compilar o ejecutar no lo genera en el árbol del proyecto. Las dependencias `path` requieren `--permitir-rutas-locales` en la descarga; git y URL no están soportadas.

## Editor

El soporte de VS Code se distribuye aparte de la CLI: actualmente mediante VSIX descargable o el paquete Debian independiente `oxido-vscode`; la publicación en Marketplace sigue pendiente. Los pasos de instalación y sus límites están en la [guía de VS Code y LSP](vscode.md). Instalar el paquete Debian de la CLI no instala ni ejecuta la extensión.

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
