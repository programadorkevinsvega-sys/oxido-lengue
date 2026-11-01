# Playground de Óxido

Servicio local que ejecuta código `.ox` dentro de un sandbox de sistema operativo. Es la implementación de la subfase 12.11, condicionada a la aprobación del producto playground.

## Responsabilidad y límites

- **Responsabilidad:** recibir una fuente acotada, ejecutarla con `oxido ejecutar` y devolver salida y diagnósticos sin exponer el host.
- **API mínima:** `GET /`, `GET /salud` y `POST /api/ejecutar` con `{"fuente":"..."}`.
- **Dependencias:** solo `serde` y `serde_json`; el aislamiento lo aportan `bwrap` y `prlimit` del sistema.
- **Crate separado:** es un servicio desplegable fuera del compilador; no forma parte de la CLI ni del LSP.

## Aislamiento

Cada ejecución usa un proceso nuevo con:

- `bwrap --unshare-all`: namespaces de red, PID, IPC, UTS y usuario; no hay red externa ni acceso al host.
- `--die-with-parent`, `--new-session` y `--cap-drop ALL`.
- Sistema de archivos de solo lectura para `/usr`, `/bin`, `/lib`, `/lib64`, `/etc/ld.so.cache`, `/etc/alternatives` y `/etc/nsswitch.conf`.
- `/tmp` y `/work` como `tmpfs` con tamaño acotado; la fuente se monta de solo lectura y el resto del host no se comparte.
- `prlimit`: 2 GiB de espacio de direcciones, 10 s de CPU, 256 procesos, 256 descriptores, 64 MiB por archivo y núcleos deshabilitados. El límite de 1 GiB se descartó tras medir que `rust-lld` no podía enlazar un programa mínimo.
- Plazo de pared de 12 s y 64 KiB por flujo de salida.
- Una sola ejecución simultánea; las demás reciben `429`.
- Solo se acepta `Content-Type: application/json`; las peticiones de formulario o `text/plain` se rechazan con `415` para evitar ejecuciones por CSRF desde otro origen.

Si `bwrap`, `prlimit` o el aislamiento no funcionan, el servicio **no arranca**. No existe una opción para desactivar el sandbox.

## Uso

```bash
cargo build --release -p oxido-playground --locked
./target/release/oxido-playground --check
./target/release/oxido-playground --bind 127.0.0.1:7878
```

`--check` comprueba las herramientas y crea un aislamiento mínimo real con `/bin/true`. Devuelve error si el sistema no permite namespaces de usuario.

## Estado verificado

- `cargo test -p oxido-playground --offline`: 13 pruebas de límites, argumentos de aislamiento, lectura HTTP acotada, rechazo de `chunked`, contenido y saneado de rutas.
- `cargo clippy -p oxido-playground --all-targets --offline -- -D warnings`: sin avisos.
- Prueba de humo `tools/playground/smoke.sh` en el job `playground-sandbox` de un runner `ubuntu-latest` de GitHub, sin contenedor privilegiado: `--check` correcto, programa ejecutado, red externa inaccesible desde el sandbox, ruta del host oculta, plazo de pared aplicado a los 12 s y `415` sin `Content-Type`.
- La misma prueba se había ejecutado antes en un contenedor Linux anidado con `--privileged`, necesario porque el host de desarrollo bloquea namespaces sin privilegios; el aislamiento interior fue el de `bwrap` en ambos casos.
- En el host de desarrollo, sin namespaces de usuario, `--check` falla de forma cerrada con código 2.

La evidencia y el método están en [`docs/reports/fase-12/2026-10-31-sandbox-linux.md`](../../docs/reports/fase-12/2026-10-31-sandbox-linux.md). La ejecución real descubrió y corrigió un `bwrap` duplicado en los argumentos, un límite de memoria demasiado bajo para enlazar Rust y un límite de 64 procesos insuficiente en el runner de GitHub.

## Limitaciones

- Solo Linux con `bwrap` y `prlimit`. No hay backend para Windows ni macOS.
- La verificación se hizo en un runner Ubuntu estándar de GitHub y en un contenedor anidado local; falta validarla en un despliegue real con usuarios.
- La imagen del sandbox debe contener un toolchain Rust compatible con `rustfmt`, un enlazador C y el binario `oxido`.
- No se ha medido el coste por ejecución ni la concurrencia de producción.
- No se publica ni se despliega; no hay autenticación porque el valor por defecto escucha en `127.0.0.1`.
- El límite de memoria del sandbox no cubre la suma de todos los `tmpfs` del host; el límite por ejecución y la exclusión mutua acotan el riesgo, pero un despliegue público requiere cuotas de disco y un supervisor externo.
