# Builds reproducibles y SBOM

**Estado:** preparado y verificado localmente en Linux; la matriz de CI está configurada, pero la firma, la notarización y la publicación no se ejecutan todavía.

## Alcance

Esta preparación cubre la parte de reproducibilidad que no requiere credenciales ni autorización de publicación:

- compilación release repetible con `SOURCE_DATE_EPOCH` y `--remap-path-prefix`;
- comparación SHA-256 de dos compilaciones independientes;
- SBOM CycloneDX por crate, sin rutas absolutas del workspace;
- preflight local de empaquetado VSIX para Linux x64;
- jobs de CI y artefactos de workflow preparados.

No incluye publicar VSIX, firmar binarios, notarizar en Apple ni usar secretos. Esas acciones requieren autorización explícita y credenciales fuera del repositorio.

## Build reproducible

```bash
tools/verify-reproducible-build.sh oxido-cli oxido release
```

El script:

1. fija `SOURCE_DATE_EPOCH=0` y `CARGO_INCREMENTAL=0`;
2. compila dos veces en directorios objetivo temporales distintos con `--locked`;
3. aplica `RUSTFLAGS="--remap-path-prefix=<workspace>=/oxido"` para eliminar la ruta del checkout;
4. compara el SHA-256 de ambos binarios y falla si difieren.

Resultado local del 2026-10-30:

```text
build reproducible: oxido-cli/oxido (release)
sha256: 90eab9164387f1237f1ad4a0e6057b5835c3f5d42110fbd96e0ac6253e02aba8
```

### Límites

- La comprobación se ejecutó en Linux `x86_64` con Rust 1.99.1. No demuestra reproducibilidad entre sistemas operativos, toolchains o máquinas distintas.
- La ruta se remapea a `/oxido`, pero el binario puede conservar otra información dependiente del entorno (por ejemplo, la versión exacta del toolchain). La comparación solo es válida con el mismo toolchain.
- No se firma el binario ni se verifica su procedencia con Sigstore.
- El job `reproducible` de CI repite la comprobación en `ubuntu-latest`; un fallo del job debe tratarse como regresión.

## SBOM

```bash
cargo install cargo-cyclonedx --version 0.5.9 --locked
tools/generate-sbom.sh
```

El script genera un CycloneDX 1.5 JSON por cada paquete del workspace (`crates/*` y `tools/*`), los renombra como `<paquete>.cdx.json`, elimina la ruta absoluta del workspace y los deja en `target/sbom/`.

- Herramienta: `cargo-cyclonedx` 0.5.9.
- Formato: CycloneDX 1.5, JSON.
- Artefacto de CI: `oxido-sbom-<sha>`, retenido 30 días.
- Los SBOM describen las dependencias del workspace; no sustituyen una auditoría de vulnerabilidades. `cargo-audit` y `cargo-deny` siguen siendo las comprobaciones de seguridad.
- El campo `metadata.timestamp` y el `serialNumber` pueden variar entre ejecuciones; el SBOM no se presenta como byte a byte reproducible.

## Preflight de VSIX

El 2026-10-31 se repitió el empaquetado local de Linux x64 sin firma ni publicación:

```bash
cargo build --release -p oxido-lsp --locked
npm ci --prefix editors/vscode-oxido
npm run package:vsix --prefix editors/vscode-oxido
OXIDO_SERVER_BINARY="$PWD/target/release/oxido-lsp" \
  OXIDO_VSIX_TARGET=linux-x64 \
  npm run package:vsix:server --prefix editors/vscode-oxido
node editors/vscode-oxido/scripts/write-sha256.js editors/vscode-oxido/dist/*.vsix
```

Resultado:

```text
3430a6ca650550062667fd9d7a5151dbb42c32e936b9aa05b342622187643c73  oxido-language-support-0.1.0-linux-x64.vsix
93767d65a463f82fe679dcdbc063add375859f26d84d5c0548f68ce5faa8862c  oxido-language-support-0.1.0.vsix
```

Los VSIX quedan en `editors/vscode-oxido/dist/`, que no se versiona. El workflow de release repite el empaquetado en Linux, Windows y macOS y adjunta los artefactos; las hashes anteriores solo demuestran que el preflight local funciona en Linux, no que el resultado sea idéntico entre plataformas.

## Workflows

- `.github/workflows/ci.yml` ejecuta:
  - `benchmark-analysis` en Linux, Windows y macOS con `--check`;
  - `reproducible` en Linux;
  - `sbom` en Linux y publica el artefacto;
  - `hygiene`, que ejecuta `tools/audit-telemetry.sh` y `tools/report-hygiene.sh docs/reports`; los jobs de validación, SBOM y benchmark aplican la misma comprobación a sus artefactos antes de subirlos.
- `.github/workflows/release.yml` comprueba reproducibilidad y genera SBOM en la ruta de release. No firma ni publica en Marketplace.
- `.github/workflows/debian-release.yml` construye y prueba paquetes CLI nativos amd64/arm64 en PRs y al crear una etiqueta de versión. También crea los VSIX autocontenidos Linux x64/arm64, Windows x64 y macOS arm64 y los paquetes Debian independientes `oxido-vscode` amd64/arm64. Antes de publicar, el job de preparación valida los cuatro paquetes Debian, los cuatro VSIX y un `SHA256SUMS` común. La prueba de CLI instala el `.deb` real, verifica la versión, instala Rust 1.99.1 en `/opt/oxido`, crea un proyecto con `oxido nuevo`, lo compila con `oxido build` y ejecuta tanto el binario como `oxido ejecutar`; después elimina el paquete. Las pruebas del nuevo paquete de editor comprueban instalación en una raíz temporal, invocación explícita de VS Code como usuario y eliminación del `.deb` sin tocar perfiles. Una etiqueta `v` coincidente con la CLI y la extensión crea o actualiza una GitHub Release con los assets; no firma paquetes ni los publica en un repositorio APT. La política está registrada en [ADR 0015](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/docs/adr/0015-distribucion-paquetes-debian.md) y [ADR 0016](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/docs/adr/0016-distribucion-extension-vscode.md).
- `.github/workflows/marketplace-publish.yml` descarga y verifica los cuatro VSIX de una Release existente. La publicación solo corre desde `main`, requiere seleccionar explícitamente `publish_to_marketplace` y tener el secreto `VSCE_PAT`. El workflow no se ha ejecutado para publicar.
- Los VSIX se construyen en runners nativos para Linux x64/arm64, Windows x64 y macOS arm64. La prueba host instalada con VS Code sigue demostrada solo en Linux x64; los builds de los demás targets no equivalen a una prueba host.

## Pendiente para publicar

| Acción | Requisito |
|---|---|
| Firmar binarios Windows | Certificado de firma de código y secreto de CI |
| Notarizar macOS | Cuenta Apple Developer, identidad y credenciales de notarización |
| Publicar VSIX | Publisher, secreto `VSCE_PAT` y activación manual explícita del workflow |
| Publicar CLI fuera de Debian | Matriz de binarios Windows/macOS y destino de publicación |
| Verificar reproducibilidad multiplataforma | Ejecutar el job en los tres sistemas y archivar hashes |

Ninguna de estas acciones se ha ejecutado.
