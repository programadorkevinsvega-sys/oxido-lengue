# Builds reproducibles y SBOM

**Estado:** preparado y verificado localmente en Linux; la matriz de CI está configurada, pero la firma, la notarización y la publicación no se ejecutan en esta fase.

## Alcance

Esta preparación cubre la parte de Fase 12.9 que no requiere credenciales ni autorización de publicación:

- compilación release repetible con `SOURCE_DATE_EPOCH` y `--remap-path-prefix`;
- comparación SHA-256 de dos compilaciones independientes;
- SBOM CycloneDX por crate, sin rutas absolutas del workspace;
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

El script genera un CycloneDX 1.5 JSON por cada paquete del workspace (`crates/*` y `tools/benchmark`), los renombra como `<paquete>.cdx.json`, elimina la ruta absoluta del workspace y los deja en `target/sbom/`.

- Herramienta: `cargo-cyclonedx` 0.5.9.
- Formato: CycloneDX 1.5, JSON.
- Artefacto de CI: `oxido-sbom-<sha>`, retenido 30 días.
- Los SBOM describen las dependencias del workspace; no sustituyen una auditoría de vulnerabilidades. `cargo-audit` y `cargo-deny` siguen siendo las comprobaciones de seguridad.
- El campo `metadata.timestamp` y el `serialNumber` pueden variar entre ejecuciones; el SBOM no se presenta como byte a byte reproducible.

## Workflows

- `.github/workflows/ci.yml` ejecuta:
  - `benchmark-analysis` en Linux, Windows y macOS con `--check`;
  - `reproducible` en Linux;
  - `sbom` en Linux y publica el artefacto.
- `.github/workflows/release.yml` añade `reproducible` y `sbom` antes de empaquetar los VSIX. El workflow sigue sin publicar ni firmar; solo adjunta artefactos.
- La matriz de release conserva Linux x64, Windows x64 y macOS arm64.

## Pendiente para publicar

| Acción | Requisito |
|---|---|
| Firmar binarios Windows | Certificado de firma de código y secreto de CI |
| Notarizar macOS | Cuenta Apple Developer, identidad y credenciales de notarización |
| Publicar VSIX | Autorización de release y token del Marketplace |
| Publicar binarios | Destino, credenciales y política de retención |
| Verificar reproducibilidad multiplataforma | Ejecutar el job en los tres sistemas y archivar hashes |

Ninguna de estas acciones se ha ejecutado.
