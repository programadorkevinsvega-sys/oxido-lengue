# VS Code y servidor LSP

El editor se compone de una extensión de VS Code (`editors/vscode-oxido`) y el servidor `oxido-lsp`. La extensión asocia archivos `.ox`, inicia el servidor y añade tres comandos. El análisis local al abrir/editar no ejecuta Cargo ni inicia `rust-analyzer`; las consultas semánticas delegadas a un bridge persistente de `rust-analyzer` y la comprobación Cargo son acciones separadas, y las operaciones que pueden ejecutar código exigen Workspace Trust.

El estado técnico y la evidencia están descritos en esta guía, la [preparación empresarial](enterprise-readiness.md) y el [ADR 0016](adr/0016-distribucion-extension-vscode.md). La extensión todavía no está publicada en Visual Studio Marketplace. La publicación se prepara como ejecución manual y requiere un publisher y el secreto `VSCE_PAT`. Los VSIX descargables no están publicados hasta crear una GitHub Release; solo Linux x64 con servidor incluido tiene prueba host de extremo a extremo.

## Requisitos

- VS Code 1.95.0 o posterior.
- `oxido-lsp` compilado del mismo commit, en `PATH` o indicado con `oxido.serverPath`, si se instala el VSIX estándar.
- El VSIX por plataforma de una GitHub Release incluye `oxido-lsp` y no requiere instalar el paquete `oxido`.
- Opcional: `rust-analyzer` y `rustfmt` del toolchain para análisis semántico y formateo.
- Opcional: un `oxido.toml` válido y un `Cargo.lock` para el comando de Cargo.

## Instalar el servidor

```sh
cargo build --release --locked -p oxido-lsp
./target/release/oxido-lsp --version
```

El servidor se comunica por stdio. No abre puertos ni envía telemetría.

## Empaquetar e instalar la extensión

```sh
npm --prefix editors/vscode-oxido ci
npm --prefix editors/vscode-oxido run package:vsix
code --install-extension editors/vscode-oxido/dist/oxido-language-support-0.1.0.vsix
```

Ese VSIX no incluye `oxido-lsp`; el servidor debe estar en `PATH` o configurarse. Para producir un paquete que incluya el servidor de la plataforma actual:

```sh
OXIDO_SERVER_BINARY=target/release/oxido-lsp \
OXIDO_VSIX_TARGET=linux-x64 \
npm --prefix editors/vscode-oxido run package:vsix:server
code --install-extension editors/vscode-oxido/dist/oxido-language-support-0.1.0-linux-x64.vsix
```

El binario incluido se usa solo si `oxido.serverPath` conserva el valor por defecto `oxido-lsp` y el archivo existe y es ejecutable. Una ruta configurada explícitamente tiene prioridad.

Desde una GitHub Release existente, descarga el VSIX cuyo sufijo corresponda a tu plataforma e instálalo manualmente:

```sh
code --install-extension oxido-language-support-0.1.0-linux-x64.vsix
```

También puedes elegir **Install from VSIX...** en la vista Extensions. Las instalaciones manuales desde VSIX no reciben actualizaciones automáticas por defecto; instala la nueva versión desde Marketplace o descarga el VSIX actualizado.

## Distribución opcional en Debian/Ubuntu

La Release prepara un paquete `oxido-vscode_<versión>-1_<arquitectura>.deb`, separado del paquete de la CLI. Incluye el VSIX Linux correspondiente y no instala `oxido`, no crea servicios y no toca perfiles de usuario. Después de instalarlo, ejecuta como el usuario que utilizará VS Code:

```sh
sudo apt install ./oxido-vscode_0.1.0-1_amd64.deb
oxido-vscode-instalar
```

También puedes elegir **Install from VSIX...** en VS Code y abrir `/usr/share/oxido-vscode/extension.vsix`. El comando instala la extensión en el perfil actual mediante la CLI oficial del editor. El `.deb` no depende de `oxido.deb` ni de cómo se instaló VS Code; `oxido-vscode-instalar` requiere que exista `code` o `code-insiders` en `PATH`. Al quitar el paquete Debian se elimina el VSIX compartido; la extensión que el usuario instaló en su perfil permanece y debe desinstalarse desde VS Code o con `code --uninstall-extension oxido-language.oxido-language-support`.

Los paquetes Debian no aparecen instalados automáticamente en los perfiles personales: VS Code mantiene sus extensiones por usuario. La instalación desde Marketplace también requiere que el usuario seleccione **Install**. En ambos casos, `oxido-lsp` se inicia solo cuando VS Code activa la extensión para archivos `.ox`.

## Marketplace

El workflow `.github/workflows/marketplace-publish.yml` solo se ejecuta manualmente desde `main`. Su opción de publicación está desactivada por defecto. Para publicar debe seleccionarse explícitamente, existir una GitHub Release con los cuatro VSIX y sus checksums, y configurarse `VSCE_PAT` junto con el publisher `oxido-language`. Ninguna publicación se ejecutó al preparar este flujo. VS Code requiere que el usuario seleccione **Install** para instalar desde Marketplace y guarda las extensiones en el perfil por usuario, como describe la [documentación oficial de Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace).

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

## Verificación

```sh
cargo test -p oxido-lsp --locked
npm test --prefix editors/vscode-oxido
npm run test:host --prefix editors/vscode-oxido
```

La suite host abre VS Code real y cubre workspace confiable, Cargo, navegación sin confianza, cancelación, VSIX instalado, VSIX con servidor incluido y Restricted Mode. Requiere `rust-analyzer` y `rustfmt`; los casos que dependen de ellos se omiten si no están disponibles.
