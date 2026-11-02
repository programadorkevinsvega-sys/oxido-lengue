# ADR 0013: cierre de 12.11 con alcance Linux

- **Estado:** aceptado
- **Fecha:** 2026-11-01
- **Contexto:** subfase 12.11, sandbox del playground

## Contexto

La subfase 12.11 se aprobó como producto condicionado: un playground que ejecuta código `.ox` ajeno. La implementación Linux usa `bwrap` y `prlimit`, no comparte red, monta `/tmp` y `/work` en `tmpfs`, limita CPU, memoria, procesos, descriptores, tamaño de archivo, plazo de pared y salida, y se niega a arrancar si el aislamiento no funciona.

La verificación de extremo a extremo se completó en un runner `ubuntu-latest` de GitHub ([run 41273981361](https://github.com/programadorkevinsvega-sys/oxido-lengue/actions/runs/41273981361), job `playground-sandbox`) y, antes, en un contenedor Linux anidado local. El job se añadió a los checks obligatorios de `main`.

Quedan tres límites que no se resolvieron:

1. No existe backend para Windows ni macOS.
2. No hay despliegue público ni usuarios reales.
3. Un despliegue requeriría autenticación, cuotas de disco y un supervisor externo.

Mantener 12.11 abierta indefinidamente impedía cerrar la Fase 12 y avanzar a documentación, aunque el alcance Linux ya estaba verificado y protegido por CI.

## Decisión

1. **Cerrar 12.11 con alcance Linux.** Los criterios de aceptación son el aislamiento del sistema operativo, la red deshabilitada, los límites de CPU, memoria, procesos y tiempo, el fallo cerrado y la prueba de humo en CI. Todos están implementados y verificados en Linux.
2. **Windows y macOS quedan fuera de 12.11.** No se declaran soportados ni pendientes dentro de esta subfase. Si el producto playground se aprueba para esas plataformas, se abrirá una subfase o un ADR nuevo con sus propios criterios y evidencia.
3. **El despliegue público queda fuera de 12.11.** El servicio escucha en `127.0.0.1` por defecto y no se presenta como listo para producción. Autenticación, cuotas y supervisor externo pertenecen a un trabajo de despliegue posterior.
4. **La evidencia y el fallo cerrado se conservan.** El job `playground-sandbox` sigue siendo obligatorio y ejecuta `--check` y `tools/playground/smoke.sh` en cada PR.
5. **No se generaliza el sandbox.** Este cierre no convierte a la CLI, al LSP ni a las compilaciones autorizadas en procesos aislados. Esos límites siguen documentados en el modelo de amenazas.

## Consecuencias

- La Fase 12 puede considerarse completada en su alcance documentado, con 12.11 limitada explícitamente a Linux.
- La documentación pública debe decir «sandbox Linux» y no «sandbox multiplataforma».
- El playground no se despliega ni se publica como servicio; hacerlo exige una decisión posterior con autenticación, cuotas y supervisor.
- Un fallo del aislamiento en Linux bloquea la fusión mediante `playground-sandbox`; un fallo en Windows/macOS no aplica porque no hay backend.
- Reabrir 12.11 para Windows/macOS requiere implementar el aislamiento específico, verificarlo en los runners correspondientes y actualizar este ADR o sustituirlo.

## Evidencia

- `tools/playground/src/sandbox.rs`: argumentos de `bwrap`/`prlimit`, límites y pruebas.
- `tools/playground/smoke.sh`: ejecución, red inaccesible, ruta del host oculta, plazo de pared y `415` sin `Content-Type`.
- Job `playground-sandbox` en verde en el run 41273981361, sin contenedor privilegiado.
- Informe canónico: [Sandbox de playground — evidencia Linux](../reports/fase-12/2026-10-31-sandbox-linux.md).
- Controles y límites: [Fase 12](../fase-12-seguridad.md).
