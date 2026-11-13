# Fase 5 — Monetización y consumo

**Estado:** Implementación funcional de catálogo, controles, suscripciones y UI en progreso de aceptación. Los precios de lanzamiento siguen sin publicar. La aceptación de Stripe sandbox queda pendiente mientras no haya claves oficiales de prueba configuradas; no se registra como exitosa por inferencia.

## Alcance 5.1

Catálogo comercial versionado, precios localizados, capacidades/cuotas por versión, trial opcional, suscripciones por workspace y sus transiciones: alta, cambio, renovación, cancelación al fin de periodo y reactivación. Prorrateo únicamente cuando el PSP y la política versionada lo permitan. Los precios de [`decision-comercial-v1.md`](decision-comercial-v1.md) —Básico USD 9/750 créditos, Pro USD 19/1.580 y Business USD 49/4.080— son hipótesis en estado borrador. No pueden publicarse ni habilitar checkout hasta superar simulación de rentabilidad aprobada y revisión tributaria/comercial.

La Fase 5.1 no implementa todavía paquetes de créditos, consumo/reservas del ledger, tarifas de inferencia, presupuestos, facturas fiscales ni conciliación integral (5.2–5.5). El otorgamiento de créditos facturados queda condicionado a confirmación firmada e idempotente del PSP; su contabilización en ledger se completa con 5.3.

## Matriz de integración obligatoria

| Frontera | Diseño y aceptación |
|---|---|
| PostgreSQL | `commercial_billing` es dueño de catálogo/versiones/precios, política de trial, suscripciones por workspace, inbox de webhooks, idempotencia y Outbox. Migraciones coordinadas y roles runtime de mínimo privilegio. No leer/escribir tablas de otros módulos. |
| Tenant HTTP/REST/OpenAPI | Catálogo disponible, estado de suscripción del workspace activo, iniciar checkout/cambio, cancelar/reactivar. No recibe datos de tarjeta; redirige al checkout alojado por Stripe cuando la tarifa esté aprobada. |
| Admin HTTP/REST/OpenAPI | Gestionar borradores/versiones/precios/capacidades, estado de publicación y simulaciones/evidencia; publicar solo con aprobación registrada. Proteger con permisos de operador de plataforma. |
| BFF → módulo | BFF es transporte REST↔gRPC, aplica cookie/CSRF/correlación y no contiene reglas de facturación ni accede a almacenes/proveedores. |
| gRPC | Contrato `CommercialBilling` en runtime para consultas/commands de tenant y plataforma; el módulo verifica actor, permiso, estado y versión. El módulo de billing consulta Tenancy Access por gRPC para validar workspace/membresía/permisos, no confía en workspace enviado por cliente. |
| Redis | Solo caché efímera del catálogo publicado y protección de abuso de checkout/webhook. TTL y reconstrucción desde PostgreSQL; Redis no es fuente de verdad. Si una operación de seguridad no puede validar el límite, falla de forma cerrada. |
| NATS JetStream / Outbox | Cambios de catálogo y ciclo de suscripción se escriben en Outbox dentro de la misma transacción de dominio; relay durable idempotente publica JetStream. Eventos versionados, sin PAN/secreto y con audiencia/scope mínimos. |
| Realtime | Eventos visibles alimentan journal durable y WebSocket común; TanStack Query tenant/admin invalida las consultas afectadas. Cambios de suscripción se enrutan al scope del workspace; catálogo publicado se enruta al scope de plataforma. Recuperación autorizada por REST/cursor y sincronización manual disponibles. No publicar datos sensibles del PSP en eventos. |
| PSP Stripe | Stripe Checkout/Subscription en USD, API alojada, claves solo en runtime. Los webhooks firmados y deduplicados son autoridad para confirmar cambios/pagos; el retorno del navegador no activa una suscripción. Idempotency-Key estable para comandos repetibles. Prorrateo deshabilitado por defecto; debe configurarse explícitamente por versión. |
| Errores/observabilidad | Errores tipados, código/categoría/dependencia/operación, SQLSTATE/constraint seguros donde existan, correlation_id/trace_id y duración; evento específico y cierre HTTP correlacionados. No registrar emails, tokens, firmas, secretos, URLs checkout ni contenido sensible. |

## Estados y reglas

- Catálogo: `draft → approved → published → retired`; no editar versiones publicadas, crear una nueva versión. Una tarifa localizada incluye moneda, monto menor, periodo, vigencia, capacidades, cuotas, referencia a Stripe Price recurrente y estado de gate de rentabilidad.
- Suscripción por workspace: `incomplete`, `trialing`, `active`, `past_due`, `canceled`, `incomplete_expired`; conservar estado local y referencias PSP. El workspace debe estar activo y el actor tener permiso `workspace.billing.manage`.
- Cobro mensual anticipado. Trial solo si la versión lo configura; no activar allowance ni acceso facturable antes de la condición autorizada. Renovación/fallo se procesa por webhook y conciliación idempotentes.
- Cancelar significa cancelar al fin del periodo pagado; reactivar antes del fin debe retirar el schedule de cancelación sin crear otra suscripción.
- Cambio sin prorrateo se programa para el próximo ciclo. Si Stripe permite y la versión aprobada lo autoriza, el cambio documenta modo de prorrateo y usa idempotencia. No cobrar en fase de borrador.
- El cambio de plan usa Stripe Subscription Schedules: mantiene el precio actual hasta `current_period_end`, aplica el siguiente precio desde ese instante y establece `proration_behavior=none`. Si el periodo no está confirmado por webhook, la moneda cambia o ya existe una incompatibilidad de estado, se rechaza con una respuesta explicativa. Cancelar libera cualquier cambio de plan pendiente y programa la cancelación al cierre del periodo.
- El límite de cambios de facturación se almacena en Redis mediante operación atómica y falla cerrado si Redis no puede comprobarlo. PostgreSQL conserva la fuente de verdad; Stripe confirma el estado comercial por webhook firmado.
- Desactivar checkout hasta verificar test credentials, webhook signing secret, Price activo/recurrente coincidente con catálogo, cuenta USD habilitada y gate fiscal. Las claves vacías no detienen runtime, pero el endpoint devuelve error estructurado de proveedor no configurado.

## Pruebas requeridas

Unitarias: transición de estados, validación/versionado de catálogo, monedas/unidades, gates que bloquean publicación/checkout, elegibilidad/estado workspace, prorrateo apagado por defecto, idempotencia, firma/replay y sanitización.

Integración con PostgreSQL/Redis/JetStream de desarrollo existentes: migración limpia y roles, CRUD de borrador/versiones, carrera de publicación/checkout, idempotencia concurrente, Outbox y replay, recuperación de caché, transiciones con eventos y scope, RBAC tenant/platform, errores módulo→gRPC→BFF HTTP con correlation/trace común y secretos ausentes en logs. Las pruebas no crean contenedores adicionales ni sustituyen servicios reales por mocks/fakes.

Aceptación de Stripe debe usar claves y webhook secrets oficiales de test. Si faltan, ejecutar la batería local real y dejar explícito el gate externo pendiente; no simular respuesta exitosa ni declarar aceptación PSP.

## Progreso

### Verificaciones realizadas

- `cargo fmt --all -- --check` y Clippy con `--all-targets -- -D warnings` en BFF, runtime, realtime y commercial-billing.
- `cargo test -p xovyra-commercial-billing`: contratos monetarios y lecturas de catálogo/suscripción en PostgreSQL real.
- `cargo test -p xovyra-realtime`: idempotencia, aislamiento de scope y proyección de eventos de facturación, archivos y trabajos en PostgreSQL real.
- `cargo test -p xovyra-module-runtime commercial_billing_service`: simulación de margen, verificación de firma, webhook firmado aplicado a PostgreSQL real, deduplicación del replay y programación sin prorrateo.
- `cargo test -p xovyra-module-runtime billing_events_route_to_the_correct_websocket_audience`: suscripciones al scope tenant y publicación de catálogo al scope de plataforma.
- `cargo test -p xovyra-module-runtime --test billing_outbox_real`: evento persistido en Outbox, publicación/lectura/replay en JetStream real, deduplicación y journal PostgreSQL por workspace.
- `cargo test -p xovyra-bff platform_catalog_routes_are_in_the_published_openapi_document`: confirma los cuatro endpoints administrativos en el OpenAPI publicado.
- Tenant: Vitest (4 pruebas), ESLint y build de producción.

| Capacidad | Estado | Evidencia |
|---|---|---|
| Matriz/documento y gates comerciales | Diseño registrado | Este documento y decisión comercial v1 |
| Persistencia/catalogo versionado y suscripciones | Implementado; migraciones 00000–00007 aplicadas en PostgreSQL de desarrollo; lectura real de catálogo y suscripción probada |
| REST/OpenAPI tenant/admin y gRPC | Implementado y registrado en OpenAPI para catálogo, checkout, estado, cancelación, reactivación, cambio de plan y gestión de versiones; `cargo check -p xovyra-bff -p xovyra-module-runtime` pasa |
| Redis, Outbox/JetStream, realtime y recuperación | Límite Redis fail-closed, cache reconstruible, Outbox con trace context, relay durable, journal con scope tenant/platform y reconexión WebSocket/TanStack Query conectados. Suscripciones se proyectan al workspace. Prueba real billing recorre Outbox→JetStream→journal, confirma replay, deduplicación y scope. Sigue pendiente recuperación automatizada tras reinicio de Redis/JetStream y verificación de invalidación WebSocket en navegador |
| Pruebas unitarias e integración real | Pasan simulación de margen, firma/replay de webhook, cambio de estado firmado e idempotente en PostgreSQL, no prorrateo, catálogo/suscripción en PostgreSQL, scope del journal y Outbox→JetStream→journal. Falta cubrir el ciclo de vida completo con Stripe sandbox y el evento hasta la invalidación observada por un cliente WebSocket autenticado |
| Stripe test end-to-end | Pendiente de credenciales sandbox y webhook secret oficiales; la API valida precios Stripe reales en las operaciones de pago |
| UI tenant/admin | Implementadas página tenant `/suscripcion` y consola admin `/plataforma/catalogo-comercial`; ambas compilan y pasan lint; cambios de suscripción invalidan sus consultas activas por WebSocket |
