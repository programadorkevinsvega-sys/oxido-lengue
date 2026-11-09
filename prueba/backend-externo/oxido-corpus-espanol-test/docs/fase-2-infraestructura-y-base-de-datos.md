# Fase 2 — Infraestructura y base de datos

## Alcance y estado

**Estado: completada.** Los gates de backend 2.1–2.4 están verificados y las dos aplicaciones frontend tienen raíces independientes en `frontend/tenant/` y `frontend/admin_plataform/`, cada una con su propio manifiesto, lockfile, configuración Vite, árbol de rutas y cliente REST centralizado. Ambas conservan por ahora la base de código inicial compartida; la depuración de rutas y funcionalidades propias de cada producto corresponde al trabajo de sus roadmaps frontend y no impide este gate de infraestructura. Este documento registra diseño, ejecución y evidencia; nunca convierte una dependencia no probada en requisito cerrado.

Requisitos normativos: [`AGENTS.md`](../AGENTS.md), [`roadmap.md`](../roadmap.md) §4, §6, §28–33, §34 criterios 1–2 y 32, y [`reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md`](../reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md) §§0–3, 6, 8–10, 12–14, 16, 19–20. Decisiones XOVYRA: [`PROJECT_RULES.md`](../PROJECT_RULES.md).

### Diagnóstico de entrada

- La revisión parte del commit `2993f37` en `main`, sincronizado con `origin/main`.
- Antes de esta fase `backend/` contenía únicamente documentación: no había workspace Cargo, código Rust, migraciones, contratos, runtime ni infraestructura de integración.
- El entorno reporta Rust/Cargo `1.99.1` estable y Docker Engine `29.9.1`. La presencia de cliente y daemon se verificará nuevamente con ejecución real al levantar PostgreSQL y NATS.
- En el cierre original de Fase 2, los 17 bounded contexts iniciales y sus schemas existían en el runtime; el contexto `realtime` se incorporó después como requisito transversal y se implementó durante las fases posteriores. El runtime actual registra 19 contextos, incluido `realtime` y `platform_control`. Se conserva una sola base física PostgreSQL y ownership de schema/migraciones por módulo.
- La disponibilidad de `protoc`, `buf`, PostgreSQL local, NATS local y otros binarios no se presupone; se verificará antes de escoger el mecanismo de generación/ejecución.

## Diseño y gates de cierre

### Fase 2.1 — Workspace Rust y ejecución

Entregables: toolchain reproducible fijado; workspace Cargo con dependencias centralizadas; crates con límites alineados con los 17 bounded contexts, `platform`, `shared-kernel`, BFF y runtime de workers/API; carga de configuración tipada desde entorno; gestión de secretos solo por referencias de entorno; logging estructurado y propagación de correlation/trace context; formato y lints sin supresiones; binarios que arrancan con configuración real y validan dependencias obligatorias.

Gate: `cargo check --workspace --all-targets`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` y arranque comprobado de los binarios. No se agregan endpoints ficticios ni defaults de secretos.

### Fase 2.2 — Persistencia compartida y migraciones

Entregables: PostgreSQL real de integración aislada; un DSN y una base física compartida; schemas/roles de mínimo privilegio separados según el ownership aprobado; migraciones versionadas en cada crate dueño; coordinador determinista, serializado y repetible; instalación desde base vacía y re-ejecución idempotente; permisos y evidencia de que la credencial de un módulo no puede modificar schemas ajenos. La instalación vacía se verifica creando una base temporal aleatoria dentro del PostgreSQL existente, migrando los 17 contextos y eliminando únicamente esa base de verificación.

Gate: ejecutar migraciones de todos los módulos contra el mismo servidor/base; inspeccionar identidad de base y ownership; reinstalar y actualizar sin duplicar ni alterar ownership; probar la frontera de permisos con sesiones reales de PostgreSQL. Las migraciones iniciales pueden crear schemas, tablas técnicas de migración y Outbox solo cuando corresponda a un contexto; no deben insertar ejemplos ni adelantarse a modelos de negocio de fases posteriores.

### Fase 2.3 — BFF y contratos internos

Entregables: BFF Axum con REST `/api/v1` y `/platform/*` separados en routing y OpenAPI; errores HTTP estables y correlation ID; contratos `.proto` versionados compilados para tonic/prost; clientes gRPC del BFF; al menos contratos técnicos reales requeridos por los procesos de Fase 2 sin simular operaciones de producto; conexión NATS JetStream real, streams declarados idempotentemente, envelope de eventos versionado y Outbox relay con publicación confirmada por JetStream antes de marcar el evento como publicado. El relay recibe un pool con permisos limitados al schema propietario; la prueba usa un evento de verificación generado por la aplicación, persistido y eliminado en su transacción temporal. Los contratos protobuf se validan con Buf CLI `1.50.0`, cuya configuración versionada aplica reglas `FILE`; la comprobación compara el árbol de trabajo con el contrato de `main` en Git y falla ante cambios incompatibles de fuente o wire.

Gate: servicio BFF invoca un servicio gRPC real por su contrato generado; OpenAPI se puede generar y sus rutas reales son comprobables; NATS JetStream persiste, entrega, reconoce y permite replay de un evento versionado; el test Outbox comprueba persistencia, publicación confirmada, entrega y estado publicado; `sh ./scripts/verify-proto-contract.sh` compila/valida el módulo protobuf con Buf y comprueba compatibilidad `FILE` contra `main`; BFF no importa crates de dominio, SQLx, NATS ni repositorios.

### Fase 2.4 — Runtime y operación base

Entregables: composición reproducible, arranque ordenado, liveness/readiness/startup según dependencias, pools y límites, timeouts configurables, propagación W3C Trace Context y correlation ID HTTP→gRPC→NATS donde aplique, logs JSON sin secretos/PII, graceful shutdown ante `SIGINT` y `SIGTERM`, y entorno real de integración reproducible. Redis es obligatorio para caché/estado técnico efímero, requiere autenticación, claves con namespace de módulo y tenant cuando aplique, TTL y no puede ser fuente de verdad. Readiness debe comprobar PostgreSQL, Redis y NATS. Los dos frontends consumen REST; no hay acceso a gRPC desde navegador. R2 sigue siendo transferencia directa navegador↔R2 y no se introduce proxy binario en backend.

Gate: arranque, estados de salud dependientes del estado real, respuesta no preparada cuando runtime/dependencia no está disponible, propagación verificable de `traceparent` y `x-correlation-id`, apagado dentro de límites ante `SIGINT`/`SIGTERM` y ejecución repetible de verificaciones con servicios PostgreSQL/Redis/NATS reales. Las pruebas no detienen dependencias persistentes: prueban degradación mediante procesos/configuración aislados y solicitudes reales. No se necesita cuenta R2 para esta fase: el contrato de frontend y la inspección de rutas confirman que aún no se implementa transporte binario backend.

## Decisiones de ejecución

1. Se trabaja secuencialmente 2.1→2.4, corrigiendo dependencias que surjan dentro de Fase 2 antes de cambiar de subfase.
2. La estructura de crates respeta los contextos existentes; los crates de producto no contendrán endpoints de negocio, modelos ficticios, semillas ni migraciones inventadas para anticipar otras fases.
3. Los contratos técnicos de plataforma no simulan capacidades de producto. Las llamadas entre BFF y módulo se ejercitan sobre gRPC real.
4. Las pruebas de infraestructura usan exclusivamente PostgreSQL, Redis y NATS reales ya levantados por el usuario y configurados en `backend/.env`. No inician, detienen, reinicializan ni eliminan contenedores o volúmenes. Cada prueba crea recursos temporales con nombres aleatorios propios en la base/namespace disponibles, los elimina al finalizar y nunca borra recursos preexistentes. No se acepta mock, fake, fixture, stub o respuesta simulada.
5. La generación de bindings gRPC debe poder hacerse de forma reproducible en un checkout limpio, con herramientas fijadas/documentadas y sin depender de artefactos locales no versionados.
6. CORS, URLs, credenciales, claves criptográficas y DSNs no reciben valores productivos por defecto; en desarrollo se cargan desde `backend/.env` ignorado por Git y en despliegue desde el gestor de secretos. Usuario y contraseña NATS se configuran en variables independientes y no se incrustan en la URL. Las variables inyectadas por el proceso prevalecen sobre el archivo local.
7. Todo contrato HTTP interno, protobuf, subject/event envelope, configuración y procedimiento operativo se documenta antes de escribir su implementación y se mantiene sincronizado.
8. Todo cambio protobuf debe superar Buf `1.50.0` con el conjunto de reglas `FILE` frente al contrato versionado en `main`. Los cambios incompatibles requieren una nueva versión de package protobuf y migración coordinada; no se desactiva ni se exceptúa una regla para hacer pasar el gate.

### Contratos técnicos de esta fase

- El runtime verificado al cerrar originalmente Fase 2 registraba 17 crates de bounded context. `realtime` y `platform_control` fueron incorporados después con contratos públicos, schema, migraciones y roles propios. El composition root enumera esos contratos; un módulo no importa internals de otro.
- Una cuenta física PostgreSQL recibe el connection pool de cada proceso y aloja los 17 schemas. Los roles `xovyra_<schema>_owner` son propietarios NOLOGIN y reciben solo el privilegio de base `CREATE` requerido para que el coordinador cree cada schema después de asumir su rol con `SET ROLE`; únicamente la cuenta migradora es miembro de esos roles. El coordinador asume uno a la vez, fija `search_path` y ejecuta el migrator SQLx de ese crate. La tabla `_sqlx_migrations` queda dentro del schema del dueño. La cuenta del runtime recibe únicamente `CONNECT`, sin permisos de lectura/escritura ni creación de schemas.
- La migración inicial de cada bounded context crea su `outbox_events`, con `event_id`, tipo/versión, fecha, payload, `trace_id` y estado de entrega. El esquema es infraestructura persistente, no datos de prueba; no introduce entidades de producto ni eventos ficticios. El relay real queda ligado a mutaciones/casos de uso de los módulos cuando sus fases los implementen; Fase 2 no afirmará entregas de eventos de negocio que aún no existen.
- El runtime implementa el contrato técnico `xovyra.runtime.v1.RuntimeReadiness/Check`. El BFF lo consume por tonic y adapta su respuesta al GET `/api/v1/system/health`; `/health/live` es local. La respuesta no incluye host, DSN, nombre de usuario, mensajes internos ni estado individual de clientes. No se habilita una ruta `/platform/*` sin autenticación de operador, que corresponde a Fase 3.
- OpenAPI enumera únicamente las rutas efectivamente montadas. No habrá rutas de producto vacías ni Swagger que sugiera módulos no implementados.
- PostgreSQL, Redis y NATS ya están levantados y se consumen desde las URLs y credenciales de `backend/.env`; Compose queda como definición declarativa, no como parte del verificador. Las contraseñas se requieren desde entorno; no hay claves default en el repositorio. El verificador no ejecuta operaciones de ciclo de vida Docker. El bootstrap PostgreSQL crea roles y grants necesarios en la base configurada; las migraciones dejan instalados los schemas de XOVYRA. Las comprobaciones temporales de Redis/NATS usan nombres aleatorios y solo eliminan los recursos que ellas mismas crearon.
- NATS usa JetStream persistente; el runtime aprovisiona un stream por bounded context con subject `<context>.>` y nombres deterministas. El mensaje de prueba de integración, si se ejecuta, lleva ID generado, se publica/consume en infraestructura real y se elimina el stream de verificación al acabar.

## Registro de ejecución

| Subfase | Estado | Evidencia / pendientes |
|---|---|---|
| 2.1 Workspace Rust y ejecución | Verificada | `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, Clippy con `-D warnings` y `cargo test --workspace` pasaron. El verificador inició BFF, migrador y runtime leyendo `backend/.env`; `.env` permanece ignorado y modo `0600`. |
| 2.2 Persistencia compartida y migraciones | Verificada | En PostgreSQL existente se re-aplicaron las migraciones de los 17 schemas. `verify-empty-database.sh` creó una base aleatoria temporal, instaló y re-ejecutó todas las migraciones, comprobó permisos/aislamiento y eliminó solo esa base. |
| 2.3 BFF y contratos internos | Verificada | BFF obtuvo readiness por tonic/gRPC y OpenAPI reflejó rutas montadas. Buf `1.50.0` compiló y pasó compatibilidad `FILE` contra `main`. Pruebas reales confirmaron persistencia/entrega/ACK/replay en JetStream y el trayecto Outbox: escritura bajo owner del módulo, publicación con ACK, estado entregado, trace headers y limpieza del row/subject aleatorios. |
| 2.4 Runtime y operación base | Verificada | Readiness consultó PostgreSQL, Redis autenticado y NATS; HTTP preservó `x-correlation-id` y `traceparent` hasta gRPC, donde el runtime validó el mismo trace ID. El verificador comprobó `SIGTERM` en runtime, `SIGINT` en BFF y degradación de readiness a 503 con liveness 200 al quedar el runtime inaccesible, sin detener dependencias persistentes. Se verificaron dos raíces de aplicación independientes (`frontend/tenant/` y `frontend/admin_plataform/`), ambas con `src/lib/api.ts` y requests REST centralizados. |

### Evidencia de entorno existente

- Se trabajó contra los contenedores preexistentes `postgres`, `redis` y `nats-jetstream-node1`; no se detuvieron ni eliminaron. El verificador actual no ejecuta `docker compose` ni comandos Docker de ciclo de vida.
- `backend/.env` contiene las credenciales locales, tiene permisos `0600` y no está versionado. Los procesos Rust cargan el archivo al arrancar; variables del proceso prevalecen.
- El bootstrap de PostgreSQL dejó los roles/privilegios y schemas XOVYRA instalados en la base indicada por `POSTGRES_DB`. No se creó otra base ni se reinicializó el volumen.
- Redis fue comprobado con autenticación y una clave temporal con TTL, eliminada al cerrar el verificador. NATS se autenticó con variables independientes y la prueba eliminó el stream aleatorio creado por ella.
- La última ejecución de `sh ./scripts/verify-phase-2.sh` pasó e incluye Buf, PostgreSQL vacío temporal, migraciones repetibles, Redis, NATS/Outbox, trazas, señales y degradación. No inicia ni elimina contenedores; los seis contenedores existentes siguen activos. Se verificó además que ambas aplicaciones frontend independientes usan el cliente REST del BFF. Cada frontend conservará su propio ámbito de producto; retirar del tenant las pantallas administrativas y del admin las pantallas de tenant, junto con completar las funciones propias de cada uno, se ejecutará dentro de sus roadmaps frontend.

## Cierre de Fase 2

- [x] 2.1 compila, formatea, pasa Clippy, ejecuta pruebas del workspace y arranca.
- [x] 2.2 instala desde una base temporal vacía, re-ejecuta las migraciones de todos los módulos y demuestra ownership/aislamiento en PostgreSQL existente.
- [x] 2.3 REST/OpenAPI, gRPC generado, protobuf compatible y NATS JetStream/Outbox tienen evidencia real y respetan límites de dependencia.
- [x] 2.4 readiness real de PostgreSQL, Redis y NATS, timeouts, trace/correlation context, apagado y degradación fueron verificados sin detener contenedores.
- [x] Existen dos aplicaciones frontend independientes en `frontend/tenant/` y `frontend/admin_plataform/`; ambas consumen REST mediante su propio `src/lib/api.ts`. La especialización de pantallas y funcionalidades sigue en los roadmaps frontend.
- [x] Documentación de contratos y operación actualizada; el cliente existente consume mediante `src/lib/api.ts`. No hay mocks, supresiones, bindings sin usar ni endpoints de demostración añadidos.
- [x] `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test --workspace`, Buf y verificaciones de integración pasaron realmente.
- [x] Cambios de cierre comprometidos en `main`, publicados en `origin/main`; árbol de trabajo limpio tras el commit.

Fase 2 queda cerrada. La separación física y técnica de los dos frontends está verificada; no se declara terminada la especialización funcional de esas aplicaciones, que continúa en sus roadmaps frontend antes de cerrar sus respectivos dominios.

### Escalabilidad horizontal y realtime transversal

La evidencia de Fase 2 corresponde al entorno local actual y una instancia de Redis/PostgreSQL/NATS; no demuestra alta disponibilidad ni operación con réplicas. No se deben codificar dependencias de memoria local, sticky sessions, nombres de host únicos ni entrega exactly-once. El diseño de producción admite Redis HA/cluster-aware, PostgreSQL primario y réplicas, JetStream en clúster con replicas de streams, múltiples runtimes/gateways/BFF y frontends stateless. Los flujos que cambien datos visibles deberán añadir Outbox/versionado y sincronización WebSocket/TanStack Query en cada subfase. Redis Pub/Sub queda como fan-out técnico efímero; JetStream/Outbox y PostgreSQL permiten reparar toda pérdida con cursor y REST. El módulo/gateway realtime compartido, sus contratos y los criterios de resiliencia horizontal están definidos en las reglas estrictas y se construyen en la fase actualmente activa; no forman parte de la evidencia ya ejecutada de Fase 2.
