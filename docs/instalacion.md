# Instalación y primeros pasos

Óxido 0.1.0 está publicado en [GitHub Releases](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0). Si vas a usar la CLI en Debian o Ubuntu, instala el `.deb` siguiendo esta página: **no necesitas clonar el repositorio ni compilar Óxido**. Los paquetes no están firmados; verifica `SHA256SUMS` antes de instalarlos. La extensión se descarga por separado; su publicación en Marketplace sigue pendiente.

## Instalar la CLI en Debian o Ubuntu

Descarga el paquete de tu arquitectura y el archivo de sumas desde la [Release v0.1.0](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0):

- [Óxido CLI para Debian/Ubuntu amd64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido_0.1.0-1_amd64.deb)
- [Óxido CLI para Debian/Ubuntu arm64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido_0.1.0-1_arm64.deb)
- [SHA256SUMS](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/SHA256SUMS)

En la terminal, entra al directorio donde guardaste ambos archivos, verifica el paquete e instálalo. Para amd64:

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./oxido_0.1.0-1_amd64.deb
oxido --version
```

Para arm64, usa `./oxido_0.1.0-1_arm64.deb` en el comando de instalación. `--ignore-missing` permite verificar el paquete descargado aunque el archivo de sumas también enumere la otra arquitectura.

La CLI necesita Rust 1.99.1 o posterior y `rustfmt` para compilar programas. Si no tienes un toolchain compatible, instálalo en el prefijo aislado de Óxido:

```sh
sudo oxido-instalar-toolchain
```

Esto no reemplaza ni desinstala el Rust del sistema o del usuario. La instalación del toolchain es explícita; `apt` no descarga Rust automáticamente. El paquete CLI se puede usar para comandos que no compilan sin instalar Rust.

Si guardaste el `.deb` dentro de `~/Descargas` y `apt` avisa que `_apt` no puede acceder al archivo, la instalación puede continuar con una descarga sin aislamiento como root. Es un aviso de permisos del directorio local, no un fallo del paquete. Para evitarlo, copia el paquete a una ruta legible por `_apt` y usa esa copia:

```sh
cp ./oxido_0.1.0-1_amd64.deb /tmp/
sudo apt install /tmp/oxido_0.1.0-1_amd64.deb
```

## Crear, compilar y ejecutar un proyecto

```sh
oxido nuevo demo
cd demo
oxido verificar
oxido compilar
oxido ejecutar
```

`oxido nuevo` crea `oxido.toml` y `src/main.ox`. Sin ruta, los comandos localizan el manifiesto y operan sobre `src/main.ox`. `compilar` genera un proyecto Cargo persistente en `target/oxido/`, con `Cargo.toml`, `src/main.rs`, los módulos y sus fuentes `.ox` (incluidos los archivos `mod.ox`), y delega la compilación en Cargo. El binario queda en `target/oxido/target/debug/oxido_generated`; `ejecutar` compila y lo ejecuta mediante Cargo. La salida esperada es `Hola, Óxido.`. Para un binario optimizado de release, ejecuta `oxido compilar --release`; el resultado queda en `target/oxido/target/release/oxido_generated`.

## ¿Quieres contribuir?

La instalación Debian ya contiene la CLI y basta para crear y compilar proyectos. Clonar el repositorio y compilar Óxido desde fuente son tareas para contribuir al compilador; están en la guía aparte [Clonar, compilar y contribuir](contribuir.md). La versión instalada se consulta con `oxido --version` o `oxido -V`.

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

Si instalaste la CLI con Debian o Ubuntu:

```sh
sudo apt remove oxido
```

Esto retira la CLI, pero conserva los datos del toolchain aislado en `/opt/oxido`. Si instalaste con Cargo, consulta los pasos de desinstalación en la [guía para contribuidores](contribuir.md).
