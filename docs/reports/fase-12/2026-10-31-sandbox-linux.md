# Sandbox de playground — evidencia Linux (2026-10-31)

**Estado:** implementación inicial verificada solo en pruebas unitarias. La ejecución aislada real no se pudo completar en este entorno porque el kernel rechaza la creación de namespaces de usuario.

## Alcance

Se implementó `tools/playground` (`oxido-playground`), un servicio local que ejecuta una fuente `.ox` con `oxido ejecutar` dentro de `bwrap` y `prlimit`. La API es `POST /api/ejecutar` con `{"fuente":"..."}`; la página `GET /` es un cliente mínimo sin recursos externos.

## Controles implementados

| Control | Implementación |
|---|---|
| Red | `bwrap --unshare-all` (incluye el namespace de red); no se comparte el host |
| Procesos | `--unshare-pid`, `--die-with-parent`, `--new-session`, `--cap-drop ALL` |
| Sistema de archivos | `/usr`, `/bin`, `/lib`, `/lib64`, `/etc/ld.so.cache`, `/etc/alternatives` y `/etc/nsswitch.conf` en solo lectura; `/tmp` y `/work` en `tmpfs`; fuente en solo lectura |
| Memoria | `prlimit --as=1073741824` (1 GiB) |
| CPU | `prlimit --cpu=10` y plazo de pared de 12 s |
| Procesos | `prlimit --nproc=64` |
| Archivos y descriptores | `--fsize=67108864`, `--nofile=256`, `--core=0` |
| Entrada | 64 KiB de fuente y 96 KiB de cuerpo HTTP |
| Salida | 64 KiB por flujo, con marca de truncado |
| Concurrencia | Una ejecución simultánea; el resto recibe `429` |
| Arranque | `--check` exige `bwrap`, `prlimit` y un aislamiento real con `/bin/true`; sin eso no escucha |

## Resultados

```text
cargo test -p oxido-playground --offline
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```text
cargo clippy -p oxido-playground --all-targets --offline -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Las pruebas cubren límites de fuente, rechazo de NUL, argumentos de aislamiento, límites de recursos, truncado y saneado de rutas, lectura HTTP acotada, rechazo de `Transfer-Encoding: chunked`, cuerpos sobredimensionados y una página autocontenida.

## Fallo cerrado

```text
$ ./target/debug/oxido-playground --check
bubblewrap no pudo crear el aislamiento: bwrap: Creating new namespace failed: Resource temporarily unavailable
exit=2
```

El entorno de desarrollo tiene `bubblewrap 0.9.0` y `prlimit`, pero no permite namespaces de usuario sin privilegios (`unshare --user` falla al escribir `uid_map`). Por eso no se ejecutó ningún programa dentro del sandbox y no se puede afirmar que el aislamiento funcione de extremo a extremo.

## Límites

- Solo Linux. No hay backend para Windows ni macOS.
- No se verificó la ejecución real, la ausencia de red desde dentro del sandbox ni el comportamiento bajo carga.
- No hay autenticación, cuotas de disco ni supervisor externo; el valor por defecto escucha en `127.0.0.1`.
- Los `tmpfs` acotan el espacio por montaje, pero el límite de memoria de `prlimit` no contabiliza toda la memoria de página del host.
- La imagen debe incluir un toolchain Rust y el binario `oxido`; no se construyó una imagen de contenedor.

## Conclusión

12.11 no se declara completada. La implementación existe, falla de forma cerrada y tiene pruebas de contrato, pero falta demostrar el aislamiento real en un host que permita namespaces y resolver Windows/macOS antes de considerarla un producto terminado.
