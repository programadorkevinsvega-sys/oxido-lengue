# Sandbox de playground — evidencia Linux (2026-10-31, actualizado 2026-11-01)

**Estado:** implementación Linux verificada de extremo a extremo en un runner Ubuntu de GitHub, sin contenedor privilegiado. No se declara cerrada: no hay backend para Windows ni macOS y falta validarla en un despliegue real.

## Alcance

Se implementó `tools/playground` (`oxido-playground`), un servicio local que ejecuta una fuente `.ox` con `oxido ejecutar` dentro de `bwrap` y `prlimit`. La API es `POST /api/ejecutar` con `{"fuente":"..."}`; la página `GET /` es un cliente mínimo sin recursos externos.

## Controles implementados

| Control | Implementación |
|---|---|
| Red | `bwrap --unshare-all` (incluye el namespace de red); no se comparte el host |
| Procesos | `--unshare-pid`, `--die-with-parent`, `--new-session`, `--cap-drop ALL` |
| Sistema de archivos | `/usr`, `/bin`, `/lib`, `/lib64`, `/etc/ld.so.cache`, `/etc/alternatives` y `/etc/nsswitch.conf` en solo lectura; `/tmp` y `/work` en `tmpfs`; fuente en solo lectura |
| Memoria | `prlimit --as=2147483648` (2 GiB de espacio de direcciones) |
| CPU | `prlimit --cpu=10` y plazo de pared de 12 s |
| Procesos | `prlimit --nproc=256` |
| Archivos y descriptores | `--fsize=67108864`, `--nofile=256`, `--core=0` |
| Entrada | 64 KiB de fuente y 96 KiB de cuerpo HTTP |
| Salida | 64 KiB por flujo, con marca de truncado |
| Concurrencia | Una ejecución simultánea; el resto recibe `429` |
| Arranque | `--check` exige `bwrap`, `prlimit` y un aislamiento real con `/bin/true`; sin eso no escucha |

## Correcciones encontradas al ejecutar

La ejecución real detectó tres defectos que las pruebas unitarias no cubrían:

1. `bind_arguments` incluía un `bwrap` de más. El comando quedaba como `prlimit … -- bwrap bwrap --unshare-all …`, de modo que el proceso exterior intentaba ejecutar `bwrap` como programa y fallaba dentro del aislamiento. Se eliminó el duplicado y se añadió una prueba que exige que `bwrap` aparezca exactamente una vez y justo después de `--`.
2. El límite de 1 GiB de espacio de direcciones impedía enlazar incluso un programa Rust mínimo: `rust-lld` abortaba con `Resource temporarily unavailable`. Se midieron 2, 4 y 8 GiB; 2 GiB fue el menor valor probado que completó la compilación (`Finished dev profile`). El límite quedó en 2 GiB y está documentado en el código.
3. El límite de 64 procesos era insuficiente en el runner de GitHub: al ejecutar `rustc` dentro del sandbox, `cargo` fallaba con `Resource temporarily unavailable (os error 11)` al hacer `exec`. Se elevó a 256, que conserva el límite y permite compilar en ese entorno.

## Verificación de extremo a extremo

### Runner Ubuntu de GitHub (2026-11-01)

Job `playground-sandbox` de la ejecución `41273981361`, sobre `ubuntu-latest`, con `bubblewrap` instalado desde apt y sin privilegios especiales de contenedor. El runner tiene red exterior; la prueba de humo levanta un servidor HTTP en el host y comprueba que el sandbox no puede alcanzarlo.

| Prueba | Resultado |
|---|---|
| `--check` | `Sandbox disponible: bwrap, prlimit y el aislamiento responden.` |
| Programa que imprime | `{"codigo":0,"duracion_ms":150,"salida":"hola desde el sandbox\n","estado":"ok"}` |
| Conexión al listener del host desde el sandbox | `SIN RED` (el listener era accesible desde el runner) |
| Lectura de `/etc/hostname` desde el sandbox | `OCULTO` |
| Programa con `sleep(30)` | `{"codigo":null,"duracion_ms":12019,"estado":"tiempo_agotado","tiempo_agotado":true}` |
| `POST /api/ejecutar` sin `Content-Type` | `415` |

### Contenedor anidado local (2026-10-31)

El host de desarrollo bloquea namespaces de usuario sin privilegios, así que la primera prueba se hizo con Docker 29.9.2 y una imagen `ubuntu:24.04` con `bubblewrap 0.9.0`, `prlimit` y `build-essential`. El contenedor exterior se ejecutó con `--privileged` para permitir namespaces anidados; el aislamiento interior siguió siendo el de `bwrap`. Se obtuvo el mismo resultado: programa ejecutado, `SIN RED`, `OCULTO` y plazo de pared de 12 s.

## Pruebas automáticas

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

La prueba de humo reproducible está en `tools/playground/smoke.sh` y se ejecuta en CI:

```sh
cargo build --locked -p oxido-cli -p oxido-playground
./target/debug/oxido-playground --check
tools/playground/smoke.sh
```

## Fallo cerrado

En el host de desarrollo, sin namespaces de usuario, el servicio sigue negándose a arrancar:

```text
$ ./target/debug/oxido-playground --check
bubblewrap no pudo crear el aislamiento: bwrap: Creating new namespace failed: Resource temporarily unavailable
exit=2
```

Esto es el comportamiento esperado: no existe un modo sin sandbox.

## Límites

- Solo Linux. No hay backend para Windows ni macOS.
- La verificación se hizo en un runner Ubuntu estándar de GitHub; no se probó un despliegue público con usuarios reales.
- No hay autenticación, cuotas de disco ni supervisor externo; el valor por defecto escucha en `127.0.0.1`.
- Los `tmpfs` acotan el espacio por montaje, pero el límite de espacio de direcciones no contabiliza toda la memoria residente del host.
- La imagen debe incluir un toolchain Rust con `rustfmt`, un enlazador C y el binario `oxido`; no se construyó una imagen de contenedor para producción.

## Conclusión

12.11 tiene evidencia real en un runner Linux estándar de ejecución, aislamiento de red, ocultación de rutas del host y plazo de pared. Sigue abierta por la ausencia de backend Windows/macOS y por no haberse validado en un despliegue real.
