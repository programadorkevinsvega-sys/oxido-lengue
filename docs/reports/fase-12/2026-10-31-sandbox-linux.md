# Sandbox de playground — evidencia Linux (2026-10-31)

**Estado:** implementación Linux verificada de extremo a extremo dentro de un contenedor anidado con namespaces habilitados. No se declara cerrada: no hay backend para Windows ni macOS y no se ejecutó en un host sin privilegios de contenedor.

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
| Procesos | `prlimit --nproc=64` |
| Archivos y descriptores | `--fsize=67108864`, `--nofile=256`, `--core=0` |
| Entrada | 64 KiB de fuente y 96 KiB de cuerpo HTTP |
| Salida | 64 KiB por flujo, con marca de truncado |
| Concurrencia | Una ejecución simultánea; el resto recibe `429` |
| Arranque | `--check` exige `bwrap`, `prlimit` y un aislamiento real con `/bin/true`; sin eso no escucha |

## Correcciones encontradas al ejecutar

La primera ejecución real detectó dos defectos que las pruebas unitarias no cubrían:

1. `bind_arguments` incluía un `bwrap` de más. El comando quedaba como `prlimit … -- bwrap bwrap --unshare-all …`, de modo que el proceso exterior intentaba ejecutar `bwrap` como programa y fallaba dentro del aislamiento. Se eliminó el duplicado y se añadió una prueba que exige que `bwrap` aparezca exactamente una vez y justo después de `--`.
2. El límite de 1 GiB de espacio de direcciones impedía enlazar incluso un programa Rust mínimo: `rust-lld` abortaba con `Resource temporarily unavailable`. Se midieron 2, 4 y 8 GiB; 2 GiB fue el menor valor probado que completó la compilación (`Finished dev profile`). El límite quedó en 2 GiB y está documentado en el código.

## Verificación de extremo a extremo

Método: host Ubuntu 24.04 con AppArmor restringiendo los namespaces de usuario sin privilegios. Se usó Docker 29.9.2 con una imagen `ubuntu:24.04` que incluye `bubblewrap 0.9.0`, `prlimit` y `build-essential`. El contenedor exterior se ejecutó con `--privileged`, `--memory 2g`, `--pids-limit 256` y `--cpus 2` para permitir la creación de namespaces anidados; el aislamiento interior siguió siendo el de `bwrap`. El contenedor exterior tenía red: una petición a `1.1.1.1` devolvió `301`.

| Prueba | Resultado |
|---|---|
| `--check` | `Sandbox disponible: bwrap, prlimit y el aislamiento responden.` |
| `GET /` | `200`, 3826 bytes |
| Programa que imprime | `{"codigo":0,...,"salida":"hola desde el sandbox\n","estado":"ok"}` |
| Conexión a `1.1.1.1:80` desde el sandbox | `SIN RED` (el contenedor exterior sí tenía red) |
| Lectura de una ruta del host desde el sandbox | `OCULTO` |
| Programa con `sleep(30)` | `{"codigo":null,"duracion_ms":12012,"estado":"tiempo_agotado","tiempo_agotado":true}` |
| `POST /api/ejecutar` sin `Content-Type` | `415` |

La ejecución se hizo con el binario de desarrollo y el toolchain Rust del host montado en solo lectura; el sandbox lo enlaza como `/rustup` y `/cargo-bin`, y compila el programa generado con `CARGO_NET_OFFLINE=true`.

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
- La verificación usó un contenedor exterior privilegiado para habilitar namespaces anidados; no se probó en un host sin Docker ni en un servicio desplegado.
- No hay autenticación, cuotas de disco ni supervisor externo; el valor por defecto escucha en `127.0.0.1`.
- Los `tmpfs` acotan el espacio por montaje, pero el límite de espacio de direcciones no contabiliza toda la memoria residente del host.
- La imagen debe incluir un toolchain Rust, un enlazador C y el binario `oxido`; no se construyó una imagen de contenedor para producción.

## Conclusión

12.11 tiene ahora evidencia real de ejecución, aislamiento de red, ocultación de rutas del host y plazo de pared. Sigue abierta por la ausencia de backend Windows/macOS y por no haberse validado en un host sin privilegios de contenedor.
