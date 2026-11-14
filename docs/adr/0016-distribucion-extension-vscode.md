# ADR 0016: Distribución opcional de la extensión de VS Code

- **Estado:** Aceptado.
- **Fecha:** 2026-11-13.

## Contexto

La CLI de Óxido y su integración de editor tienen ciclos de instalación y dependencias distintas. Instalar la CLI no debe instalar VS Code ni activar el servidor LSP. El servidor compilado puede distribuirse dentro del VSIX para que la extensión funcione sin instalar la CLI.

## Decisión

- La extensión se publica en Visual Studio Marketplace cuando el autor configure el publisher y `VSCE_PAT`, y dispare explícitamente el workflow de publicación. El flujo queda manual y su opción de publicación está desactivada por defecto.
- GitHub Releases ofrece VSIX autocontenidos para Linux x64/arm64, Windows x64 y macOS arm64, y un `.deb` `oxido-vscode` independiente para Debian amd64/arm64.
- `oxido-vscode` instala el archivo VSIX compartido en `/usr/share/oxido-vscode/extension.vsix` y un comando `oxido-vscode-instalar`. El comando instala la extensión en el perfil del usuario que lo ejecuta mediante `code --install-extension`; no se ejecuta como script mantenedor ni modifica perfiles durante instalación/eliminación.
- El paquete Debian no depende de `oxido.deb`; contiene `oxido-lsp` dentro del VSIX. No instala un daemon. VS Code inicia el servidor al activar la extensión para archivos `.ox`.
- `SHA256SUMS` cubre ambos paquetes CLI, ambos paquetes de extensión y los cuatro VSIX antes de publicar una GitHub Release. La Release no se marca pública hasta que todas las matrices y checksums pasan.

## Consecuencias

El usuario puede instalar desde Marketplace, descargar e instalar un VSIX manualmente, o instalar el `.deb` opcional y ejecutar `oxido-vscode-instalar`. Al eliminar el `.deb`, se retira su copia compartida del VSIX, pero una instalación ya copiada al perfil del usuario permanece y debe desinstalarse desde VS Code.

El Marketplace y la extensión aún no se publican con este cambio. La operación requiere cuenta/publisher y token fuera del repositorio. Los `.deb` no son una extensión global activada automáticamente en todos los perfiles; VS Code documenta las extensiones como instaladas por usuario. Solo Linux x64 tiene actualmente prueba host instalada de extremo a extremo.
