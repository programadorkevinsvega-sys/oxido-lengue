# VS Code y servidor LSP

El editor se compone de una extensión de VS Code (`editors/vscode-oxido`) y el servidor `oxido-lsp`. La extensión asocia archivos `.ox`, inicia el servidor y añade tres comandos. El análisis local al abrir/editar no ejecuta Cargo ni inicia `rust-analyzer`; las consultas semánticas delegadas a un bridge persistente de `rust-analyzer` y la comprobación Cargo son acciones separadas, y las operaciones que pueden ejecutar código exigen Workspace Trust.

El estado técnico y la evidencia están descritos en esta guía, la [preparación empresarial](enterprise-readiness.md) y el [ADR 0016](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/docs/adr/0016-distribucion-extension-vscode.md). La [Release v0.1.0](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0) ya ofrece los paquetes `.deb` y VSIX que se enlazan abajo. La extensión todavía no está publicada en Visual Studio Marketplace; su publicación requiere configurar publisher y `VSCE_PAT` y ejecutar el workflow manual. Solo Linux x64 con servidor incluido tiene prueba host de extremo a extremo.

## Requisitos

- VS Code 1.95.0 o posterior.
- `oxido-lsp` compilado del mismo commit, en `PATH` o indicado con `oxido.serverPath`, si se instala el VSIX estándar.
- El VSIX por plataforma de una GitHub Release incluye `oxido-lsp` y no requiere instalar el paquete `oxido`.
- Opcional: `rust-analyzer` y `rustfmt` del toolchain para análisis semántico y formateo.
- Opcional: un `oxido.toml` válido y un `Cargo.lock` para el comando de Cargo.

## Instalar desde la Release

Descarga el VSIX correspondiente a tu plataforma desde la [Release v0.1.0](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0). Cada paquete incluye el servidor `oxido-lsp`; no tienes que clonar el repositorio, compilar la extensión ni instalar la CLI de Óxido. Descarga también [SHA256SUMS](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/SHA256SUMS) para verificar el archivo; los paquetes no están firmados.

| Plataforma | Descarga |
|---|---|
| Linux x64 | [VSIX](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-language-support-0.1.0-linux-x64.vsix) |
| Linux arm64 | [VSIX](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-language-support-0.1.0-linux-arm64.vsix) |
| Windows x64 | [VSIX](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-language-support-0.1.0-win32-x64.vsix) |
| macOS arm64 | [VSIX](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-language-support-0.1.0-darwin-arm64.vsix) |

Descarga el archivo y después instálalo desde la terminal:

```sh
code --install-extension oxido-language-support-0.1.0-linux-x64.vsix
```

También puedes elegir **Install from VSIX...** en la vista Extensions y seleccionar el archivo descargado. Las instalaciones manuales desde VSIX no reciben actualizaciones automáticas por defecto; para actualizar, descarga e instala la nueva versión desde una Release posterior.

## Distribución opcional en Debian/Ubuntu

El paquete `oxido-vscode` es opcional y separado del paquete de la CLI. Incluye el VSIX Linux correspondiente, no instala `oxido`, no crea servicios ni toca perfiles de usuario durante las operaciones de `apt`. Descarga la versión adecuada de la [Release v0.1.0](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/tag/v0.1.0):

- [Extensión VS Code para Debian/Ubuntu amd64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-vscode_0.1.0-1_amd64.deb)
- [Extensión VS Code para Debian/Ubuntu arm64](https://github.com/programadorkevinsvega-sys/oxido-lengue/releases/download/v0.1.0/oxido-vscode_0.1.0-1_arm64.deb)

Después de instalarlo, ejecuta como el usuario que utilizará VS Code:

```sh
sudo apt install ./oxido-vscode_0.1.0-1_amd64.deb
oxido-vscode-instalar
```

También puedes elegir **Install from VSIX...** en VS Code y abrir `/usr/share/oxido-vscode/extension.vsix`. El comando instala la extensión en el perfil actual mediante la CLI oficial del editor. El `.deb` no depende de `oxido.deb` ni de cómo se instaló VS Code; `oxido-vscode-instalar` requiere que exista `code` o `code-insiders` en `PATH`. Al quitar el paquete Debian se elimina el VSIX compartido; la extensión que el usuario instaló en su perfil permanece y debe desinstalarse desde VS Code o con `code --uninstall-extension oxido-language.oxido-language-support`.

Los paquetes Debian no aparecen instalados automáticamente en los perfiles personales: VS Code mantiene sus extensiones por usuario. La instalación desde Marketplace también requiere que el usuario seleccione **Install**. En ambos casos, `oxido-lsp` se inicia solo cuando VS Code activa la extensión para archivos `.ox`.

## Marketplace

El workflow `.github/workflows/marketplace-publish.yml` solo se ejecuta manualmente desde `main` y su opción de publicación está desactivada por defecto. Marketplace aún no está publicado: requiere configurar `VSCE_PAT` y el publisher `oxido-language`, y luego ejecutar el workflow explícitamente. Mientras tanto, instala desde uno de los VSIX enlazados arriba o desde el paquete Debian opcional. Cuando esté en Marketplace, VS Code requerirá que el usuario seleccione **Install** y guardará la extensión en su perfil, como describe la [documentación oficial de Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace).

## Configuración

| Ajuste | Valor por defecto | Efecto |
|---|---|---|
| `oxido.serverPath` | `oxido-lsp` | Ruta al ejecutable del servidor. En Restricted Mode solo se acepta la configuración de usuario |

La extensión declara `untrustedWorkspaces.supported = "limited"` y restringe `oxido.serverPath` en modo restringido. La configuración del workspace solo selecciona el modo de sintaxis y la edición; no contiene comandos.

## Comandos

| Comando | Qué hace | Confianza |
|---|---|---|
| `Óxido: Reiniciar servidor de lenguaje` | Reinicia el proceso LSP | No requiere |
| `Óxido: Comprobar con Cargo` | Ejecuta una comprobación real con el `oxido.toml` y el `Cargo.lock` del proyecto | Requiere Workspace Trust |
| `Óxido: Ver estadísticas del editor` | Muestra duración, round-trip y memoria residente | No requiere |

`Óxido: Comprobar con Cargo` admite una sola comprobación concurrente y `$/cancelRequest` mata el árbol de procesos. El resultado cancelado no publica diagnósticos. El comando usa `--locked --offline`; si faltan dependencias o el lockfile, devuelve el diagnóstico correspondiente.

## Capacidades

- Diagnósticos léxicos y estructurales con spans convertidos a UTF-16.
- Semantic tokens para declaraciones, referencias y ámbitos locales de documentos abiertos.
- Símbolos de archivos cerrados desde una caché versionada fuera del workspace.
- En workspace confiable, un bridge persistente delega hover, completado, inlay hints, definición, referencias, semantic tokens y renombrado en `rust-analyzer`, con correspondencia exacta de ubicaciones para las regiones verificadas.
- En Restricted Mode, navegación y renombrado local con el índice y los ámbitos locales, sin iniciar `rust-analyzer`.

No se afirma mapeo semántico completo entre módulos ni resolución de tipos universal. Las ediciones sin correspondencia exacta se rechazan.

## Límites y seguridad

- El LSP no inicia Cargo ni `rust-analyzer` automáticamente al abrir o editar; las solicitudes semánticas explícitas pueden iniciar y mantener el bridge de `rust-analyzer` en workspace confiable.
- El comando Cargo puede ejecutar `build.rs` y macros procedurales de dependencias. El Workspace Trust expresa confianza; no es un sandbox.
- La compilación autorizada no está aislada del sistema operativo. El sandbox Linux del playground es un producto aparte y no cubre la CLI ni el LSP.
- El framing admite 4 MiB; la cabecera, 8 KiB; 64 documentos y 16 MiB en total.
- El índice escanea como máximo 256 archivos, 2048 entradas, 4 MiB por archivo y 16 MiB de caché, y nunca escribe dentro del workspace.
- La memoria residente solo se informa en Linux. En Windows y macOS el valor es `null` y la extensión muestra «no disponible».
- El Marketplace no está publicado y la matriz Windows/macOS del VSIX con servidor todavía no tiene prueba host de extremo a extremo.

## Solución de problemas

| Síntoma | Causa probable | Acción |
|---|---|---|
| «No se encontró oxido-lsp» | El binario no está en `PATH` | Configura `oxido.serverPath` con una ruta absoluta |
| El servidor arranca y se cierra | Versión incompatible o error de protocolo | Reinicia y revisa la salida del canal de la extensión |
| `OX4001` al formatear | Falta `rustfmt` | Instala el componente del toolchain |
| Sin funciones de Rust | `rust-analyzer` ausente o workspace no confiable | Instala el componente y concede confianza |
| El comando Cargo no hace nada | Restricted Mode | Concede Workspace Trust de forma explícita |
| `OX7001` | Falta `Cargo.lock` | Descarga y confirma las dependencias antes de compilar |

## Pruebas de la extensión (contribuidores)

```sh
cargo test -p oxido-lsp --locked
npm test --prefix editors/vscode-oxido
npm run test:host --prefix editors/vscode-oxido
```

La suite host abre VS Code real y cubre workspace confiable, Cargo, navegación sin confianza, cancelación, VSIX instalado, VSIX con servidor incluido y Restricted Mode. Requiere `rust-analyzer` y `rustfmt`; los casos que dependen de ellos se omiten si no están disponibles.

## Compilar desde fuente (contribuidores)

Los comandos siguientes sirven para desarrollar la extensión y el servidor; no son necesarios para instalar el VSIX publicado. Para clonar y preparar el repositorio completo, sigue [Clonar, compilar y contribuir](contribuir.md).

```sh
cargo build --release --locked -p oxido-lsp
./target/release/oxido-lsp --version
npm --prefix editors/vscode-oxido ci
npm --prefix editors/vscode-oxido run package:vsix
```

El servidor se comunica por stdio, no abre puertos ni envía telemetría. El VSIX estándar generado desde fuente no incluye `oxido-lsp`; el servidor debe estar en `PATH` o configurarse. Para generar un VSIX autocontenido de Linux x64:

```sh
OXIDO_SERVER_BINARY=target/release/oxido-lsp \
OXIDO_VSIX_TARGET=linux-x64 \
npm --prefix editors/vscode-oxido run package:vsix:server
```

El servidor incluido se usa solo si `oxido.serverPath` conserva el valor por defecto `oxido-lsp` y el archivo existe y es ejecutable. Una ruta configurada explícitamente tiene prioridad.
