# Reglas específicas del proyecto XOVYRA Backend

## Autoridad y alcance

- El alcance funcional obligatorio está en [`roadmap.md`](roadmap.md).
- El contrato arquitectónico universal está en [`reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md`](reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md). [`ARCHITECTURE_RULES.md`](ARCHITECTURE_RULES.md) es su copia de referencia para las herramientas que esperan ese nombre; mantenla idéntica al contrato canónico.
- Las instrucciones de ejecución del agente están en [`AGENTS.md`](AGENTS.md).
- Este archivo fija las decisiones específicas de XOVYRA. No puede contradecir ni reducir los documentos anteriores.

## Identidad del proyecto y objetivo

XOVYRA es un backend SaaS global, multiusuario y multimodal. Los tenants crean y gestionan proyectos y activos de texto, imagen, video, clips, voz, música y combinaciones; usan proveedores externos con capacidades verificadas; administran equipos, uso y facturación; y pueden conectar canales de publicación. El flujo financiero/audiovisual para YouTube es una capacidad opcional.

## Stack y topología acordados

| Área | Decisión del proyecto |
|---|---|
| Lenguaje y async | Rust estable, versión fijada; Tokio |
| Front door | BFF HTTP con Axum, REST versionado y OpenAPI |
| Comunicación interna síncrona | gRPC con tonic/prost mediante contratos versionados |
| Comunicación interna asíncrona | NATS JetStream con async-nats; eventos versionados y Outbox |
| Persistencia | Una base física PostgreSQL compartida; un schema lógico y ownership de tablas/migraciones por módulo; SQLx |
| Cache | Redis solo como dependencia auxiliar justificada, nunca como fuente de verdad |
| IA inicial | Workers AI con los 10 modelos `@cf/...` seleccionados y catálogo Cloudflare AI/AI Gateway con FLUX 3 Image Klein, P-Image, P-Video y MiniMax H3 Max de terceros; adapters server-side, capacidades y tarifas verificadas por separado |
| Correo | Resend para correo transaccional y campañas autorizadas; adapter server-side en Notifications, credenciales fuera del repositorio, webhooks autenticados e idempotencia |
| Objetos | Cloudflare R2 privado; control/autorización en backend y bytes directos navegador↔R2 |
| Multimedia | FFmpeg y FFprobe; scratch local temporal, archivos permanentes en R2 |
| Observabilidad | tracing + OpenTelemetry; métricas, logs y traces sin secretos/PII |
| Despliegue inicial | Monolito modular en el despliegue inicial. BFF y runtime de módulos pueden ser procesos separados en la misma infraestructura; no desplegar microservicios independientes como requisito inicial. |

El BFF solo adapta REST a contratos públicos gRPC y no contiene reglas de negocio ni acceso a PostgreSQL/repositorios. Los módulos no importan internals de otros módulos. El frontend tenant y el frontend de administración son aplicaciones distintas, consumen REST/OpenAPI y tienen sesiones/autorización separadas.

### Integración obligatoria en cada fase y subfase

Desde el diseño y la primera implementación de cada fase/subfase se construye el recorrido real completo: frontend tenant/admin → HTTP/REST/OpenAPI → BFF → gRPC → bounded context dueño. Los frontends nunca se conectan directamente a gRPC, módulos, PostgreSQL, Redis o NATS. El BFF adapta HTTP y gRPC sin reglas de negocio ni acceso directo a almacenes.

Cada documento de fase/subfase debe especificar, antes del código, migraciones PostgreSQL del módulo, contratos REST/OpenAPI, contratos gRPC, uso de Redis y eventos NATS JetStream/Outbox. Redis y NATS se integran en esa misma entrega cuando el dominio los requiere. Una decisión de no aplicabilidad debe tener justificación técnica explícita y verificable; no basta con omitirlos. Redis conserva únicamente caché/estado técnico efímero con TTL. NATS JetStream conserva eventos durables asíncronos publicados mediante Outbox cuando hay consistencia con escrituras; no sustituye RPCs ni la fuente de verdad PostgreSQL. La subfase se prueba de extremo a extremo a través del BFF y sus contratos reales.

### Pruebas obligatorias por fase y errores diagnosticables

Cada fase y subfase incluye pruebas unitarias para reglas puras y pruebas de integración/aceptación automatizadas para los flujos reales afectados, desde HTTP del frontend/BFF hasta gRPC, módulo dueño y dependencias aplicables. No se declara backend/BFF verificado solo por compilar ni se sustituye este recorrido por pasos manuales en navegador. Las pruebas usan PostgreSQL, Redis y JetStream reales existentes cuando correspondan, limpian sus propios datos y no reinician procesos manuales ni crean contenedores.

Para cada error, la aceptación comprueba que el fallo operativo específico y el evento de cierre HTTP se correlacionan mediante `correlation_id` y `trace_id`, y que el cierre conserva el código/categoría del fallo en vez de reducirlo a un status o `unclassified_http_failure`. También verifica el contrato público seguro y la ausencia de secretos/PII en logs y respuestas. Todo error debe indicar operación, componente/dependencia, causa clasificada y duración cuando estén disponibles; `retryable` solo se afirma con evidencia. Si una capa pierde el contexto, la fase no se cierra hasta corregirla o registrar claramente el bloqueo verificable.

## Bounded contexts propuestos

Este es el mapa de propiedad vigente. Cada fila corresponde a un crate de módulo y a un schema propio dentro de la misma base PostgreSQL. `realtime` ya está implementado como bounded context del monolito modular, con schema/migraciones, API gRPC, gateway WebSocket y worker JetStream; el composition root registra 19 contextos. Las llamadas a otro contexto son por gRPC o eventos NATS; no se permiten JOINs, FK ni escrituras entre schemas de módulos.

| Crate / bounded context | Responsabilidad exclusiva | Schema PostgreSQL |
|---|---|---|
| `identity` | Usuarios, autenticación, sesiones, verificación y recuperación | `identity` |
| `tenancy_access` | Workspaces/tenant, membresías, invitaciones y RBAC de workspace/plataforma | `tenancy_access` |
| `commercial_billing` | Planes/versiones, suscripciones, facturas, pagos y webhooks comerciales | `commercial_billing` |
| `usage_ledger` | Créditos, reservas/liquidaciones, cuotas, uso/costes imputados y tarifas comerciales aplicadas | `usage_ledger` |
| `provider_catalog` | Proveedores/modelos disponibles, capacidades verificadas, parámetros y tarifas base versionadas | `provider_catalog` |
| `integrations` | Conexiones por workspace, OAuth, referencias a secretos externos, webhooks y estado | `integrations` |
| `projects` | Proyectos multimodales, series, campañas, plantillas, activos y configuración general | `projects` |
| `financial_research` | Briefs financieros, fuentes, afirmaciones, cifras, verificación y procedencia | `financial_research` |
| `generation` | Solicitudes y resultados de generación multimodal independientes vía adapters habilitados | `generation` |
| `asset_storage` | Metadatos/ACL/versiones de archivos, ciclo de vida y autorización de operaciones R2 | `asset_storage` |
| `jobs` | Trabajos, etapas, dependencias, leases, retries, scheduling técnico y progreso | `jobs` |
| `media_pipeline` | Escenas, composición, TTS/transcripción según capacidades, FFmpeg, renders y QC | `media_pipeline` |
| `approvals` | Revisiones y aprobación inmutable de versiones exactas | `approvals` |
| `publishing` | Canales de publicación, calendario editorial, YouTube y reconciliación remota | `publishing` |
| `analytics` | Métricas/reportes de canal y agregaciones de producto con procedencia | `analytics` |
| `notifications` | Preferencias, entregas, estados y errores de notificación | `notifications` |
| `realtime` | Journal/cursor de sincronización, autorización de suscripciones y gateway WebSocket modular | `realtime` |
| `audit` | Registro inmutable de acciones relevantes y accesos de soporte | `audit` |

Los nombres son de contexto y no de capa técnica. `apps/bff`, `apps/api`, `platform` y `shared-kernel` son componentes de composición/infraestructura, no bounded contexts con propiedad de datos de negocio. La consola global es un plano de acceso de operador que invoca contratos de módulos; no obtiene tablas ni una membresía implícita de tenant.

## Propiedad de persistencia y migraciones

- Todos los módulos usan la misma instancia/base física PostgreSQL.
- Cada crate solo crea y cambia sus objetos del schema asignado.
- Las migraciones se versionan en cada módulo y un coordinador las aplica en orden determinista al instalar o actualizar.
- No hay transacciones que crucen módulos, JOINs o FKs entre schemas; las referencias externas conservan IDs y se validan por contrato.
- El Outbox vive en el schema del módulo productor y se publica a NATS JetStream después de persistir el cambio de negocio.
- Los roles/permisos base requeridos para operar se instalan por migración del módulo dueño; no se cargan datasets de demostración.

## Contratos de integración iniciales

- Cliente tenant: REST versionado bajo `/api/v1`; cada operación con workspace valida membresía, recurso y permiso en servidor.
- Operación global: `/platform/*`, autenticación y permisos de operador separados.
- Errores REST: código de dominio estable, mensaje seguro, detalles de validación y correlation/request ID; OpenAPI es la fuente para generar/validar clientes.
- Errores globales: en cada fase y subfase registrar contexto interno suficiente para diagnóstico (`bounded_context`, componente, operación, dependencia, clase/código, resultado, `correlation_id`/`trace_id` y metadatos técnicos seguros disponibles) y devolver detalles seguros/accionables en el contrato público. Nunca registrar ni exponer secretos, credenciales, tokens, cookies, URLs firmadas, contenido privado, datos sensibles innecesarios, SQL crudo o stack traces. La aceptación de cada dominio incluye el recorrido de error módulo→gRPC→BFF→REST→UI y sus estados de recuperación.
- Un registro final que solo diga `HTTP 503` o `gRPC Unavailable` incumple la regla. Produce eventos correlacionados por frontera (inicio, llamada, fallo causal clasificado, traducción y resultado), con duración independiente por dependencia y duración total. Extrae `traceparent`/correlation ID en cada servicio gRPC y hereda esos campos en los logs del handler. No inventes causa, dependencia ni retryability: registra `unknown`/`unclassified` hasta tener evidencia y mejora el punto donde se pierde el dato.
- Llamadas internas síncronas: `.proto` versionados, DTOs públicos y contexto de request/trace/tenant; nunca tipos internos de dominio.
- Eventos: envelope versionado con `event_id`, `event_type`, `event_version`, `occurred_at`, `producer_module`, `trace_id` y contexto de tenant cuando aplique; consumidores idempotentes.
- Eventos realtime: todas las mutaciones visibles se guardan con Outbox en la transacción dueña, se distribuyen por JetStream y llegan al gateway modular para WebSocket/BFF; TanStack Query invalida/refresca por coordinador central. Incluye audience/scope, `event_id`, `stream_cursor`, versión de agregado, recuperación REST y deduplicación. Aplica a todas las fases/subfases de tenant y plataforma.
- Gateway: `realtime` es bounded context del monolito modular, con el mismo ownership, capas, migraciones, `module_public_api` gRPC, configuración y observabilidad que los demás módulos; no es un microservicio ni conexión directa desde navegador. Redis Pub/Sub HA solo distribuye señales efímeras entre réplicas.
- Trabajos: respuestas iniciales incluyen ID/estado real; progreso y errores se consultan mediante REST/WebSocket protegido según contrato.
- Archivos: el backend valida intención, autorización, propiedad, cuotas y finalización; emite autorización temporal para objeto/método exacto. Los bytes se transfieren directamente entre navegador y R2.
- Capacidades IA: únicamente catálogo verificado por modelo/versión; el frontend nunca infiere modalidades ni recibe credenciales permanentes.

## Decisiones comerciales específicas

- Los canales iniciales de inferencia son distintos: Workers AI para los 10 IDs Cloudflare `@cf/...` alojados por Cloudflare y AI Gateway/catálogo Cloudflare para los modelos terceros `black-forest-labs/flux-3-image-klein`, `pruna/p-image`, `pruna/p-video` y `minimax/h3-max`. Mantener adapters, unidades, billing, cuotas, capacidades y conciliaciones separados; los terceros no son Workers AI ni consumen la cuota de neurons. El detalle versionado está en [`docs/decision-comercial-v1.md`](docs/decision-comercial-v1.md).
- Resend es el proveedor de correo para verificación, avisos de cuenta, recuperación, recordatorios y campañas. Las campañas promocionales requieren consentimiento explícito, baja sencilla y respeto de supresiones; correo transaccional de seguridad no depende de consentimiento promocional. Nunca afirmar entrega sin aceptación del proveedor.
- Cloudflare R2 es el almacenamiento privado canónico.
- La política comercial normativa está en [`docs/decision-comercial-v1.md`](docs/decision-comercial-v1.md); backend, tenant y admin deben implementar el mismo catálogo, ledger, impuestos, comisiones y política de retención.
- La experiencia comercial de generación usa créditos; el backend los calcula a partir del precio del modelo y sus unidades oficiales de consumo, con tarifas y conversión de crédito versionadas. Los créditos no sustituyen el registro monetario ni la trazabilidad del coste.
- La meta inicial es 30 % de margen operativo antes del impuesto a la renta, si costos variables y asignados están medidos. Para costo del proveedor `C`, costos operativos asignados `O`, comisión fija de cobro asignada `F`, comisión porcentual `r` y margen `m`, el precio antes de impuestos es `P=(C+O+F)/(1-r-m)`. No llamar margen neto ni garantizarlo si hay costos no asignados; mostrar por separado margen bruto de inferencia, contribución, operativo y resultado neto.
- Wompi procesa COP en Colombia. El propietario confirma que ya tiene una cuenta Stripe para USD internacional; usarla tras validar estado merchant, país, liquidación, métodos, tarifas, credenciales y webhooks, sin volver a tratar la existencia de cuenta como pendiente. El impuesto se añade según reglas fiscales configuradas; el tenant paga el total aplicable y XOVYRA conserva responsabilidad de cálculo/cumplimiento. No asumir automáticamente IVA 19 %.
- Los créditos son unidades internas de consumo, no unidades/costo de Cloudflare ni dinero. Mantener valor de referencia configurable/versionado (USD 0,01 inicial) y cobrar operaciones como `ceil(P/valor_referencia_credito)`. Catálogo de hipótesis: Básico USD 9/750 créditos, Pro USD 19/1.580, Business USD 49/4.080; paquetes Mini USD 5/400, Plus USD 15/1.250, Studio USD 50/4.200. No publicar esos valores antes de simulación con tarifas reales y escenarios de consumo. Créditos no pagan R2.
- Simular rentabilidad de cada plan con escenarios de uso, mezcla de modelos, expiraciones, costos asignados, cobro PSP, reembolsos/contracargos y límites. Los cargos PSP fijos se registran en la transacción y se distribuyen para análisis del plan/paquete; nunca se cargan una vez por generación ni se cuentan dos veces. Alertar o bloquear catálogos por debajo de margen mínimo configurado.
- Aplicar grants mensuales tras confirmación del pago, reintentos de pago en días 1/3/7 y gracia de 7 días; créditos mensuales vencen a los 60 días, créditos comprados no vencen mientras la cuenta siga activa (sujeto a ley). Consumo FIFO de los que vencen primero. No borrar saldo ni historial: corregir mediante asientos inversos.
- El almacenamiento persistente R2 es una opción de pago del cliente, activada explícitamente y financiada con un saldo monetario de almacenamiento independiente de los créditos de generación. El cargo se deriva de uso atribuible versionado por propietario —GB-mes y operaciones R2 facturables, además de retrieval si aplica— con la misma meta operativa inicial del 30 % sobre el precio final, si los costos asociados están asignados y conciliados. El uso temporal necesario para una operación tiene retención limitada y no habilita almacenamiento permanente sin opt-in.
- R2 persistente: recarga manual mínima equivalente a USD 5; auto-recarga desactivada por defecto. Reservar proyección de 30 días, medir/liquidar diariamente, avisar con 20 % de saldo o siete días estimados. Agotado el saldo: 7 días de gracia y después 30 días de solo lectura/exportación antes del borrado, con avisos y auditoría. Sin persistencia, los resultados temporales expiran a las 24 horas. La política detallada está en el documento comercial.
- Usar las tarifas unitarias oficiales versionadas para calcular precios de cliente sin trasladar los free tiers/créditos globales de la cuenta Cloudflare; conciliar por separado el cargo real agregado del proveedor y los beneficios de cuenta. Guardar tarifa/versión/unidad/moneda/costo/margen/precio aplicado sin recalcular transacciones previas.
- Coste operativo Cloudflare, consumo medido por XOVYRA, créditos del cliente, saldo de almacenamiento y cargos comerciales son libros/valores distintos y conciliables.
- El workspace es el límite inicial de tenant y facturación según el roadmap; cualquier jerarquía empresarial adicional requiere decisión explícita y definición de ownership.
- YouTube y el flujo financiero son integraciones/workflows opcionales, no requisitos para generar activos multimodales generales.

## Restricciones de calidad

Cumplir las prohibiciones del contrato estricto y `AGENTS.md`: no código sin usar; ninguna supresión de diagnósticos; ningún `_`/`_variable` usado para evadir código sin usar; ningún mock, fixture, stub, seed de prueba, dataset de ejemplo o respuesta simulada; probar con servicios/adapters reales en infraestructura aislada o sandbox oficial. No se declaran capacidades, operaciones o fases completas sin evidencia real.

## Decisiones pendientes, no inventar

Estas definiciones no se fijan en esta fase. Deben resolverse y documentarse antes de la subfase que las necesite:

| Decisión/insumo | Requerido antes de | Responsable/entrada pendiente |
|---|---|---|
| Clasificación tributaria, facturación electrónica, tratamiento de créditos y reglas por país | Fase 5.2 y antes de producción | Validación por asesor tributario; el diseño no sustituye asesoría legal/fiscal |
| Cuenta/contrato Wompi, tarifas vigentes y configuración de cuenta Stripe existente (estado, país, monedas, credenciales/webhooks) | Fase 5.2 | Cuentas/contratos y verificación técnica; no se considera pendiente la existencia de Stripe |
| Proveedor de correo transaccional, dominios y política de entrega | Fase 3.2 | Cuenta, DNS y credenciales |
| Cuenta Cloudflare, API/token, AI Gateway/Unified Billing o BYOK, modelos habilitados y límites | Fase 9.1–9.4 | Cuenta/token con privilegio mínimo y verificación oficial |
| Cuenta/bucket R2, CORS de producción, lifecycle y política final de retención | Fase 10.2–10.4 | Cuenta, bucket y orígenes de los dos frontends |
| Proyecto OAuth de Google/YouTube, scopes, cuota y estado de verificación | Fase 13.1–13.4 | Cuenta/proyecto y credenciales de sandbox/operación autorizada |
| Infraestructura objetivo, dominios, TLS, secretos y política de despliegue | Fase 17.1–17.2 | Decisión operativa y accesos de infraestructura |
| Collector/retención de telemetría y SLOs iniciales | Fase 15.4–15.5 | Entorno de observabilidad y objetivos de negocio aprobados |
| Catálogo COP/USD definitivo, asignaciones operativas, escenarios de rentabilidad y cuotas por plan/país | Fase 5.1/5.4 | Finanzas configura y aprueba la versión antes de cobro real |
| Política fiscal validada por jurisdicción y vigencia | Fase 5.2 | Asesoría tributaria y alta de reglas en backend |

La falta de estas entradas no impide cerrar el diseño de Fase 1, pero bloqueará el cierre de las subfases posteriores afectadas hasta tenerlas.
