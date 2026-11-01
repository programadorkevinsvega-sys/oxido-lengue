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
- `prlimit`: 1 GiB de espacio de direcciones, 10 s de CPU, 64 procesos, 256 descriptores, 64 MiB por archivo y núcleos deshabilitados.
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
- `--check` falla de forma cerrada en el entorno de desarrollo actual porque el kernel no permite crear namespaces de usuario (`Resource temporarily unavailable`). Por tanto, la ejecución aislada de extremo a extremo **no está verificada aquí**.

## Limitaciones

- Solo Linux con `bwrap` y `prlimit`. No hay backend para Windows ni macOS.
- La imagen del sandbox debe contener un toolchain Rust compatible y el binario `oxido`.
- No se ha medido el coste por ejecución ni la concurrencia de producción.
- No se publica ni se despliega; no hay autenticación porque el valor por defecto escucha en `127.0.0.1`.
- El límite de memoria del sandbox no cubre la suma de todos los `tmpfs` del host; el límite por ejecución y la exclusión mutua acotan el riesgo, pero un despliegue público requiere cuotas de disco y un supervisor externo.
