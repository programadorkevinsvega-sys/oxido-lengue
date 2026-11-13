# ROADMAP MAESTRO — BACKEND INTEGRAL DE XOVYRA

## Regla arquitectónica obligatoria

La implementación debe cumplir el documento [reglas estrictas de arquitectura de monolito modular Rust](reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md) en todas sus fases y dominios. Consúltalo antes de diseñar o modificar código y úsalo como referencia normativa para límites de módulos, persistencia, migraciones, contratos, seguridad y operación. Este roadmap define las capacidades de producto; no autoriza excepciones a esas reglas. Si detectas un conflicto o una instrucción que no pueda cumplirse junto con el documento, detén únicamente la parte afectada, explica el conflicto y propone una solución compatible antes de implementarla.

### Regla global de errores y diagnóstico

Aplica en **todas las fases, subfases y dominios**. Cada fallo produce eventos estructurados relacionados para entrada, llamada a dependencia, causa clasificada, traducción gRPC/HTTP y finalización. Cada evento incluye nombre estable, servicio/componente, operación, clase/código de error, dependencia/resultado cuando se conozcan, duración, `correlation_id` y `trace_id`, además de metadatos técnicos seguros (SQLSTATE, códigos HTTP/gRPC/proveedor, constraint/tabla, timeout/reintento). Un 503 final sin causa, un mensaje genérico o `error_type` no satisfacen esta regla. La propagación de contexto entre BFF, gRPC, módulo, almacenes y eventos debe estar comprobada; las llamadas a dependencias deben medir duración por separado del tiempo HTTP total. REST/gRPC presentan un contrato tipado con mensaje claro, detalles seguros y referencia de solicitud; indica recuperación, campo a corregir o espera cuando aplique. No expongas ni registres secretos, cookies, tokens, credenciales, URLs firmadas, contenido privado, PII innecesaria, SQL crudo o stack traces. Cada fase debe incluir pruebas automatizadas de clasificación, propagación y redacción desde el módulo hasta BFF y frontend. La prueba de aceptación correlaciona cada cierre HTTP fallido con su error específico por ambos identificadores (`correlation_id` y `trace_id`) y confirma que el código/categoría se conserva; probar solo el status final no es suficiente. Cada fase también añade pruebas unitarias y de integración de las fronteras reales aplicables y no deja la aceptación backend/BFF dependiente de un recorrido manual en navegador.

## 1. Rol y objetivo

Actúa como arquitecto de software sénior, ingeniero experto en Rust, SaaS multi-tenant, sistemas distribuidos, procesamiento multimodal, integración con APIs de inteligencia artificial, PostgreSQL y seguridad.

XOVYRA es una plataforma SaaS global, multiusuario y multimodal para crear, editar, organizar y gestionar contenido digital mediante proveedores externos de inteligencia artificial. Debe admitir imágenes, videos, clips, audio, narraciones, música, texto y proyectos combinados; espacios de trabajo personales y de equipo; administración de recursos y consumo; múltiples modelos de IA; e integraciones de publicación. Los flujos especializados, como la producción automatizada de videos financieros para YouTube, son capacidades opcionales y no limitan el producto.

Diseña, implementa, prueba y documenta un backend de producción que provea contratos seguros y completos para el frontend de cliente y la consola global de plataforma. La investigación financiera, el flujo editorial audiovisual y YouTube se conservan como dominios especializados.

**No construyas un MVP, prototipo, maqueta, backend de demostración ni colección de endpoints sin lógica real.** Implementa todas las funcionalidades definidas en este documento mediante fases sucesivas hasta completar el sistema.

No te limites a entregar recomendaciones o fragmentos de código: trabaja sobre el proyecto real, crea los archivos necesarios, ejecuta las pruebas, corrige los errores y entrega un sistema funcional.

## 2. Reglas obligatorias de ejecución

1. Inspecciona primero todo el repositorio, sus archivos, dependencias, estructura, configuraciones, migraciones SQL, servicios y funcionalidades existentes.
2. Si el repositorio ya contiene código, consérvalo cuando sea válido. No destruyas funcionalidades ni reemplaces componentes sin justificarlo.
3. Antes de modificar archivos, presenta un diagnóstico breve de la arquitectura actual y un plan completo de implementación.
4. Divide el trabajo en fases, pero completa todas las fases. No te detengas al terminar la primera.
5. No marques una función como terminada si solamente existe su estructura, una respuesta simulada o un TODO.
6. No inventes APIs, endpoints, parámetros, capacidades o precios de proveedores externos. Verifica sus contratos oficiales antes de implementar cada integración.
7. Si falta una credencial, una cuenta externa o una decisión indispensable, implementa la integración completa hasta donde sea posible, documenta el bloqueo y solicita únicamente la información necesaria. No simules una integración real como si funcionara.
8. No almacenes secretos en el código, el repositorio, los logs ni el frontend.
9. Todas las operaciones críticas deben tener validaciones, autorización, manejo de errores, auditoría y pruebas.
10. No publiques contenido real durante las pruebas. Usa los entornos oficiales de prueba/sandbox de los proveedores y canales, con la integración real; no sustituyas servicios por mocks o simulaciones. Si el proveedor no ofrece sandbox o faltan credenciales, documenta el bloqueo y no declares validada esa operación.
11. Mantén documentación actualizada conforme implementes el sistema.
12. Al finalizar cada fase, ejecuta las pruebas correspondientes y corrige los errores antes de continuar.

## 3. Objetivo funcional y volumen

Como perfil opcional de producción editorial, la plataforma debe poder gestionar, mediante configuración:

- 15 videos Shorts diarios.
- 2 videos largos diarios.
- Duración configurable de videos largos, inicialmente entre 5 y 10 minutos.
- Duración configurable de Shorts.
- Generación concurrente controlada.
- Varias series de contenido y campañas.
- Uno o varios canales de YouTube, con aislamiento de datos y permisos.
- Programación de publicaciones por fecha, hora y zona horaria.
- Presupuestos diarios, mensuales y por proyecto.
- Crecimiento futuro sin reescribir toda la arquitectura.

El volumen anterior es la capacidad objetivo de planificación, no una obligación de consumir créditos o generar contenido cuando se alcance el límite de presupuesto.

Ese perfil especializado debe permitir modificar la cantidad de publicaciones, las duraciones, las cuotas de concurrencia y el presupuesto. No impongas esos objetivos ni canales como requisito para proyectos multimodales generales.

## 4. Arquitectura tecnológica obligatoria

### Backend

- Rust estable, con versión fijada en el proyecto.
- Axum para HTTP y API REST.
- `tonic`/`prost` para contratos gRPC síncronos entre el BFF y los módulos, y entre módulos cuando una respuesta inmediata sea necesaria.
- NATS JetStream con `async-nats` para eventos asíncronos durables entre módulos; usa Outbox para publicar eventos de forma consistente con PostgreSQL.
- Tokio para concurrencia y operaciones asíncronas.
- SQLx para acceso tipado a PostgreSQL y migraciones.
- Una única base de datos PostgreSQL compartida físicamente por todos los módulos. Cada módulo posee su schema lógico, tablas y migraciones; se prohíbe el acceso cruzado y los JOINs entre módulos.
- Serde para serialización.
- Validación de entrada con bibliotecas mantenidas y compatibles.
- OpenAPI mediante una herramienta compatible con Axum.
- Trazas estructuradas con `tracing`.
- FFmpeg y FFprobe para procesamiento audiovisual.
- OAuth 2.0 para integraciones que lo requieran.
- Workers de Rust para tareas de larga duración.
- Sistema persistente de trabajos, reintentos, bloqueos, recuperación y programación.

Redis es una dependencia auxiliar obligatoria para caché y estado técnico efímero, detrás de puertos de caché del módulo que lo utilice. Toda clave debe tener TTL, namespace del módulo y, cuando contenga datos de cliente, el identificador interno del workspace. Redis nunca es fuente de verdad de negocio ni el único lugar donde se almacena el estado de trabajos: PostgreSQL debe permitir recuperar su ejecución después de reiniciar el servidor. Una indisponibilidad de Redis debe producir degradación/errores explícitos según la operación, nunca una pérdida silenciosa de escrituras persistentes.

### Actualización realtime transversal (tenant y administración)

Toda mutación persistida que afecte datos visibles a otros usuarios o sesiones debe producir un evento Outbox transaccional. JetStream distribuye los eventos durables a módulos y proyecciones; un módulo/gateway `realtime` integrado al monolito modular distribuye cambios a los clientes por WebSocket público del BFF y contrato gRPC interno. No es un microservicio ni una implementación particular por pantalla. El sistema se aplica transversalmente a tenant y plataforma, con audiencias, sesiones, permisos y namespaces estrictamente independientes.

Cada módulo publica envelope versionado con `event_id`, `stream_cursor`, tipo/versión, scope/audiencia, agregado y versión monotónica, timestamps y correlation ID. El payload público solo identifica los dominios/recursos que deben refrescarse; el cliente vuelve a consultar el dato por REST/OpenAPI con permisos vigentes. TanStack Query se integra con un único coordinador realtime central por aplicación que asigna eventos a familias de query keys y deduplica/reordena versiones. No se implementa lógica de socket separada por vista.

WebSocket se autentica en el BFF con cookie HttpOnly y Origin exacto; no usa tokens en URL ni reemplaza los endpoints REST. Incluye reconexión con jitter y cursor de reanudación. REST proporciona snapshot/delta de recuperación desde un cursor duradero para desconexiones, huecos, duplicados, versiones fuera de orden, overflow o eventos incompatibles. Se mantiene acción de sincronización manual como reparación/rollback de UI. No se confía en el socket como fuente de verdad ni como confirmación de una escritura.

La topología debe tolerar varias instancias desde el diseño: frontends stateless servidos en varias réplicas/CDN; BFF stateless y sin sticky sessions obligatorias; runtime/gateway replicable; JetStream en clúster con streams replicados, consumidores durables idempotentes y despliegues con versiones mixtas; Redis HA/cluster-aware para fan-out técnico efímero entre gateways y límites/cache con TTL; PostgreSQL primario para escritura y réplicas para lectura. Redis Pub/Sub puede perder señales y nunca guarda la única copia; cursor/REST recupera desde PostgreSQL. Las lecturas tras una mutación respetan read-your-writes y posición de replicación para no ocultar cambios por lag. Ninguna réplica puede depender de memoria local para autorización, eventos pendientes o estado de negocio.

**Criterio transversal obligatorio:** la matriz y los criterios de aceptación de cada fase/subfase deben indicar las mutaciones visibles, su Outbox/evento, consumidores JetStream, audience/scope, actualización WebSocket/TanStack Query y recuperación REST. Esto incluye identidad/cuenta, tenancy/equipos, facturación/uso, proveedores/generación, archivos/R2, trabajos, publicación, analítica, configuración, notificaciones y consola de plataforma. Las fases cerradas se revisan al integrar sus dominios al sistema realtime; un cierre histórico no autoriza flujos aislados.

### Infraestructura

La API y los workers de XOVYRA se ejecutarán en la infraestructura elegida para el proyecto. Los modelos alojados por Cloudflare se consumen por Workers AI con IDs `@cf/...`; los modelos terceros se consumen por catálogo Cloudflare AI/AI Gateway y conservan facturación/proveedor diferenciados. Cloudflare R2 será el almacenamiento canónico de objetos. El servidor puede usar disco local solo como scratch temporal de procesamiento, con límites y limpieza, nunca como autoridad o ubicación permanente de los archivos del usuario.

Implementa un monolito modular Rust: módulos como crates con límites de dominio, un BFF Axum como adaptador REST, y contratos gRPC/NATS para respetar las fronteras aun cuando se desplieguen juntos. Debe poder funcionar inicialmente en un único servidor y permitir separar posteriormente API, workers y procesamiento multimedia. El BFF solo adapta REST a gRPC; no contiene lógica de negocio ni accede a PostgreSQL, Redis o repositorios de módulos.

No despliegues microservicios independientes como requisito inicial. Conserva un único despliegue modular si resulta adecuado, sin saltarte por ello los contratos gRPC y eventos NATS establecidos en el documento [reglas estrictas de arquitectura](reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md).

### Límites de módulos y persistencia

- Cada dominio implementa su lógica y persistencia en su propio crate; ningún módulo importa entidades, casos de uso o repositorios internos de otro.
- Todos los módulos usan la misma instancia/base física PostgreSQL, con un schema y propiedad de tablas definidos por módulo. No crees una base de datos por módulo.
- Cada módulo mantiene migraciones versionadas independientes; el despliegue las aplica de forma coordinada a la base compartida. Un módulo nunca modifica tablas de otro y no hay JOINs SQL entre módulos.
- Para comunicación síncrona usa el contrato gRPC del módulo destino. Para comunicación asíncrona usa eventos versionados de NATS JetStream y Outbox. Redis se usa únicamente como caché/estado técnico efímero con claves aisladas y TTL; nunca es fuente de verdad.
- El BFF REST es el único contrato HTTP de aplicación para los dos frontends; ofrece APIs de cliente y `/platform/*` con autenticación/autorización independientes. Ambos frontends nunca acceden directamente a gRPC, NATS, PostgreSQL ni proveedores.
- La transferencia binaria R2 conserva su flujo directo navegador↔R2 con autorizaciones temporales emitidas por el backend; el BFF controla permisos, cuotas y ciclo de vida, pero no retransmite los bytes.

## 5. Diseño del almacenamiento Cloudflare R2

R2 es el almacén privado para activos persistentes de usuarios que hayan activado y financiado explícitamente el almacenamiento. Los archivos temporales requeridos para ejecutar o entregar una operación se guardan solo bajo una política de retención acotada, se cobran dentro del costo de esa operación o del servicio según su origen y se eliminan al vencer; no equivalen a almacenamiento persistente. Usa el API compatible con S3 desde servicios backend para operaciones administrativas y para firmar autorizaciones. Ningún bucket privado se publica ni se sirve con acceso anónimo.

### Transferencia directa navegador ↔ R2

Los bytes de carga y descarga se transfieren directamente entre navegador y R2; no pasan por el servidor de API de XOVYRA. El backend sigue controlando completamente el plano de control y el ciclo de vida:

1. El cliente solicita a la API una intención de carga indicando recurso, tamaño esperado, tipo y checksum cuando esté disponible.
2. El backend autentica la sesión, valida membresía, permisos, opt-in y saldo monetario de almacenamiento, límites de tamaño/tipo, reserva el cargo estimado y genera una autorización temporal, firmada para un solo objeto/operación y alcance mínimo. La ausencia de saldo no puede consumirse de los créditos de generación.
3. El navegador transfiere directamente a R2 usando esa autorización. Las credenciales permanentes de R2 nunca se entregan al cliente.
4. El cliente notifica la finalización; el backend verifica el objeto en R2 (existencia, tamaño, checksum/ETag aplicable y metadatos), valida contenido real mediante el procesamiento requerido y marca el recurso disponible solo después de superar las validaciones.
5. Para descargar, la API vuelve a comprobar identidad, estado del recurso, espacio de trabajo, propietario/ACL y permisos; luego emite una autorización temporal GET del objeto exacto. El navegador descarga directamente desde R2.

La carga se hace primero a una clave aleatoria de staging/quarantena, distinta de la clave final. Al completar, valida el staging y promueve/copia el objeto a una clave final nueva y no reutilizable; el recurso disponible apunta solo a esa clave final. Nunca permitas que una URL PUT aún vigente pueda sobrescribir un objeto ya validado/visible.

Trata cada URL firmada como un bearer token que permite la operación especificada mientras siga vigente. Usa expiraciones breves y configurables (por ejemplo, un máximo de cinco minutos), objeto y método exactos, no las registres, no las incluyas en analítica ni mensajes de error, evita redirecciones abiertas y emítelas solo tras autorizar cada solicitud. Una URL emitida puede ser compartida y reutilizada hasta expirar; documenta esta ventana residual y no la describas como URL de un solo uso o revocable instantáneamente. Si hace falta revocación inmediata, elimina/rota el objeto o cambia a un mecanismo de autorización que la soporte antes de prometerla. Sigue la [documentación oficial de URLs prefirmadas de R2](https://developers.cloudflare.com/r2/api/s3/presigned-urls/) y su guía de [cargas directas desde navegador](https://developers.cloudflare.com/r2/objects/upload-objects/).

Configura CORS de R2 con una allowlist exacta de orígenes de producción, métodos, headers requeridos y exposición mínima. CORS limita navegadores, no sustituye autenticación ni autorización. No uses un bucket público, dominio público, URL permanente o listado de objetos para recursos privados. Usa transporte HTTPS y políticas de origen seguras.

Para archivos grandes, implementa carga multipart directa con sesiones y partes limitadas, URLs firmadas por parte, expiración, límites, finalización validada y abort de cargas abandonadas. No emitas credenciales amplias de bucket al navegador. Si se usan credenciales temporales, deben estar limitadas al tenant, prefijo/objeto, operaciones y duración estrictamente necesarios.

### Propiedad, aislamiento y validación

- Los object keys los genera el servidor con IDs no predecibles; nunca aceptes una ruta/key del navegador como autoridad. No incorpores nombres de archivo como rutas.
- Cada objeto tiene tenant/espacio de trabajo, propietario, recurso y política de acceso persistidos en PostgreSQL; el bucket y el prefijo no reemplazan el control de autorización.
- Por defecto un archivo privado solo es accesible por su propietario y permisos autorizados. Los archivos compartidos de equipo requieren ACL explícita. Un miembro no puede acceder a archivos privados de otro usuario ni a objetos de otro espacio, aunque conozca su ID o key.
- Revalida autorización al iniciar, completar, consultar, descargar, compartir, borrar y procesar una carga. Workers y tareas programadas deben conservar el tenant y la autorización del recurso.
- Valida tamaño real, checksum y tipo detectado por contenido, no solo `Content-Type` del cliente. Mantén cargas en cuarentena/no disponibles hasta validarlas, escanearlas cuando aplique y asociarlas a un recurso autorizado.
- Usa checksum criptográfico, metadatos/versiones, escrituras completas verificadas, limpieza de multipart incompletos y reconciliación de objetos huérfanos. No confíes en ETag como checksum universal.
- Mantén binarios fuera de PostgreSQL; conserva metadatos, objeto interno, dueño, ACL, versiones, estados, hashes, cuotas y trazabilidad. Nunca devuelvas bucket, credenciales, keys administrativas ni rutas internas.
- Mantén scratch local acotado para FFmpeg u otras tareas; elimina temporales tras uso y vuelve a R2 los resultados finales mediante credenciales de worker con privilegio mínimo.

### Cuotas, facturación y ciclo de vida

- El almacenamiento R2 persistente es opt-in, se activa después de informar la tarifa, unidad y periodo, y se financia con un saldo monetario separado. Mide por workspace y propietario los bytes y duración (GB-mes), versiones y objetos billables; contabiliza también operaciones Clase A/B y retrieval cuando apliquen. No debites créditos de generación por almacenamiento ni saldo de almacenamiento por inferencia.
- La factura R2 de Cloudflare corresponde a la cuenta de XOVYRA y no atribuye consumos por tenant. Mide y atribuye el uso por objeto/propietario; cobra al dueño del espacio personal o al responsable de facturación del equipo. Conserva separados el costo proveedor, el cargo comercial y el saldo de almacenamiento. Usa tarifa oficial unitaria versionada antes de aplicar free tiers globales; concilia diferencias y beneficios de cuenta contra la factura agregada de Cloudflare. El egreso directo R2, si Cloudflare lo mantiene gratis, no genera una línea de costo R2.
- Apunta a un margen operativo del 30 % sobre el precio final antes de impuestos, condicionado a que costos estén medidos/asignados. Para costo Cloudflare `C`, costo operativo asignado `O`, comisión fija de pago atribuida `F`, tasa PSP `r` y margen operativo objetivo `m`, usa `P = (C + O + F) / (1 - r - m)`. Incluye comisiones PSP no recuperables una sola vez. No llamar margen neto al objetivo si hay gastos no asignados; el resultado neto depende también de impuestos sobre la renta y contabilidad completa. Gastos compartidos no atribuibles por operación se cubren con suscripciones/planes o una política explícita y no se presentan como cubiertos por el margen bruto.
- Reserva el saldo monetario estimado al activar/renovar almacenamiento y al iniciar cargas para evitar carreras; liquida por medición real tras verificar el objeto y libera reservas ante fallos. Sin opt-in/saldo suficiente no se habilita persistencia ni se firma la carga. Ante saldo agotado, bloquea nuevas altas/cargas persistentes, notifica y aplica una política versionada de gracia/retención/eliminación que se defina antes de producción.
- Los activos temporales de una generación sin almacenamiento persistente tienen retención acotada y expiración automática; su duración, costo incluido y recuperación deben quedar definidos antes de implementar los flujos de generación y R2. No se promete conservación indefinida ni se exige activar almacenamiento R2 para consumir créditos de generación.
- Expón uso actual, estimado/facturable, historial, límites y alertas en contratos de API; distingue medición estimada de cargos conciliados.
- Implementa retención configurable, eliminación autorizada, borrado lógico cuando corresponda, eliminación física controlada, lifecycle, objetos huérfanos y auditoría. Conserva límites legales/comerciales configurados.
- Incluye respaldos de metadatos y objetos con controles de acceso, retención y restauración probada; evita que el rol de backup abra acceso de descarga a usuarios.

## 6. Organización del código

Diseña una estructura modular por dominios. Puedes adaptarla a las necesidades del proyecto, pero debe contemplar:

- `config`
- `bootstrap`
- `api`
- `auth`
- `users`
- `roles`
- `permissions`
- `channels`
- `projects`
- `content`
- `research`
- `sources`
- `scripts`
- `ai_providers`
- `workspaces`
- `memberships`
- `invitations`
- `billing`
- `subscriptions`
- `credit_ledger`
- `capabilities`
- `integrations`
- `public_api`
- `generation`
- `media`
- `rendering`
- `subtitles`
- `thumbnails`
- `quality_control`
- `review`
- `approvals`
- `publishing`
- `scheduling`
- `analytics`
- `costs`
- `budgets`
- `notifications`
- `jobs`
- `storage`
- `audit`
- `health`
- `metrics`

Separa los controladores HTTP, los servicios de negocio, los modelos, los repositorios y las integraciones externas.

Define contratos e interfaces internas para los proveedores de IA. No acoples la lógica de negocio a una marca específica.

## 7. Autenticación, sesiones y seguridad

Implementa un sistema de autenticación real con:

- Inicio de sesión.
- Cierre de sesión.
- Cierre de todas las sesiones del usuario.
- Renovación segura de sesión.
- Expiración por inactividad y expiración absoluta.
- Revocación de sesiones.
- Cambio de contraseña.
- Recuperación de contraseña mediante mecanismo seguro.
- Verificación de correo obligatoria para activar una cuenta pública; Resend envía correo de verificación y avisos transaccionales usando el remitente verificado `XOVYRA <no-reply@jnmtortas.com>` en el despliegue actual. Mantener el nombre y la dirección alineados con el dominio autenticado; esto no garantiza la bandeja de entrada. Los errores de entrega se informan sin revelar si la cuenta existe.
- Protección contra fuerza bruta.
- Limitación de intentos.
- Invalidación de sesiones después de eventos de seguridad relevantes.
- Listado de sesiones activas y sus metadatos no sensibles.
- Protección contra CSRF cuando corresponda.
- Cookies `HttpOnly`, `Secure` y `SameSite` adecuadas si se usa autenticación basada en cookies.
- CORS restringido a los orígenes configurados.
- Contraseñas con hash resistente, por ejemplo Argon2id.
- Tokens de recuperación de un solo uso, almacenados de forma segura.
- Registro de eventos de autenticación.

Para un frontend web propio, prioriza sesiones revocables con cookies seguras. No almacenes tokens de acceso en `localStorage` si puedes evitarlo.

Implementa RBAC con permisos granulares.

Roles iniciales de espacio de trabajo (no son roles de operadores de plataforma):

- Superadministrador.
- Administrador.
- Director de contenido.
- Investigador.
- Guionista.
- Editor audiovisual.
- Revisor.
- Publicador.
- Analista.
- Lector.

Define por separado los roles y permisos de plataforma, limitados a tareas de operación global. No reutilices el rol de superadministrador como bypass implícito de autorización de recursos de cliente.

Permite crear roles personalizados y asignar permisos sin modificar código.

Aplica autorización en cada operación del servidor. Ocultar un botón en el frontend no constituye una medida de seguridad.

Separa la identidad global de la persona de su autorización en cada espacio de trabajo. Un rol global de operador pertenece al plano de plataforma y nunca debe convertirse implícitamente en un rol dentro de tenants. Los roles de equipo se evalúan junto con membresía activa, alcance del recurso y política del dominio.

## 8. SaaS multi-tenant y espacios de trabajo

Define el aislamiento como una propiedad obligatoria del dominio y de la base de datos, no como un filtro opcional del frontend.

Define un único límite canónico de tenant para datos y cobro. En el producto inicial, el espacio de trabajo es ese tenant/organización y su ID es la clave de aislamiento y suscripción; no crees una tabla `organizations` paralela con semántica ambigua. Si una organización empresarial necesita varios espacios, documenta y aplica explícitamente la jerarquía organización → workspace y determina en cada dominio cuál es propietario, ámbito de autorización y entidad facturable.

- Modela usuarios globales, espacios de trabajo, membresías, roles y permisos propios de cada espacio.
- Cada entidad de cliente (proyectos, archivos, proveedores, trabajos, consumos, integraciones y configuraciones) debe tener un propietario/ámbito de espacio de trabajo explícito.
- Deriva el espacio activo de una membresía autenticada y autorizada; nunca confíes en un `workspace_id` arbitrario del cuerpo de la petición.
- Autoriza cada lectura, escritura, descarga, evento WebSocket y operación de worker contra el espacio y los recursos asociados.
- Aísla claves de objeto, cachés, eventos, búsquedas, trabajos, temporales y metadatos R2 por identificadores internos de tenant; prevén IDOR y acceso cruzado.
- Define restricciones e índices compuestos y, cuando convenga, Row Level Security como defensa adicional. La autorización de aplicación sigue siendo obligatoria.
- Al cambiar o revocar una membresía, aplica el cambio en solicitudes y sesiones posteriores; audita invitaciones, roles y cambios de acceso.
- Permite varios espacios por usuario, creación del primero durante onboarding y estados de membresía pendiente, activa, suspendida y revocada.
- Proporciona una resolución consistente del contexto del espacio para API, workers, tareas programadas y operaciones administrativas.

XOVYRA tiene **dos aplicaciones frontend distintas**: `frontend/tenant/` para clientes/tenants y `frontend/admin_plataform/` para el admin de plataforma. No son dos áreas, modos o rutas de un mismo frontend. Cada una debe poder compilarse y desplegarse de forma independiente, con punto de entrada, configuración, autenticación/sesión, shell, rutas y navegación propios. Durante la preparación inicial pueden partir del mismo código, pero sus roadmaps frontend deben retirar las rutas y funcionalidades del otro producto antes de declarar terminados sus dominios. Pueden estar en repositorios separados o como apps distintas de un monorepo; solo comparten contratos API o paquetes neutrales sin sesión, navegación ni UI de aplicación.

Una vez especializada cada aplicación conforme a su roadmap, tenant no incorpora pantallas, menús ni enlaces de navegación al admin y solo consume las APIs de cliente/workspace; admin se limita al plano de control `/platform/*`, con autorización de operador independiente. La base de código inicial compartida no concede acceso: backend mantiene autorización y sesiones independientes desde el primer día. El operador no obtiene por ello membresía ni permisos administrativos dentro de espacios de cliente. Toda acción de soporte con acceso a datos de cliente debe ser explícita, autorizada, limitada y auditada.

### Administración SaaS global

Implementa para operadores autorizados una vista global de organizaciones/espacios, estado de clientes y membresías, planes y suscripciones, facturas y pagos, créditos/consumo, almacenamiento R2 atribuido, límites, uso por modelo/proveedor, integraciones, incidencias y estado del servicio. Incluye búsqueda, filtros, historial y exportación autorizada. Agrega métricas globales sin revelar contenido privado ni secretos.

Permite administrar catálogo de planes, tarifas Cloudflare versionadas, objetivo de margen operativo inicial del 30 % con costos asignados, escenarios de rentabilidad, valor de créditos, saldo/política de almacenamiento R2, cuotas y modelos/capacidades habilitados. Cada cambio debe tener permisos de plataforma, validación, auditoría, vigencia efectiva e historial; los nuevos precios no reescriben transacciones pasadas. Las acciones de soporte sobre un tenant requieren acceso temporal explícito y auditado y nunca crean una membresía de cliente.

## 9. Registro y onboarding

Completa la autenticación con flujos de alta pública y configuración inicial:

- Registro con validación, protección contra abuso y respuestas que no permitan enumerar cuentas.
- Verificación de correo con tokens de un solo uso, expiración, reenvío limitado y revocación segura.
- Creación del primer espacio de trabajo y su membresía propietaria en una operación consistente e idempotente.
- Flujos recuperables para onboarding incompleto, correo pendiente y creación duplicada/concurrente.
- Selección de plan durante o después del alta según la política comercial y la respuesta del servicio de facturación.
- Endpoints para consultar y guardar pasos/preferencias iniciales sin convertirlos en autoridad de permisos.
- Integración con Resend para correo transaccional, notificaciones y recordatorios, con estados de entrega/error observables, firma de webhooks e idempotencia; no afirmar éxito si el proveedor falla. El remitente transaccional configurado en el despliegue actual es `XOVYRA <no-reply@jnmtortas.com>` y debe estar verificado en Resend. La dirección exacta del remitente es parte de la identidad de envío, pero no asegura que Gmail clasifique el mensaje como correo deseado. Las promociones/campañas requieren consentimiento explícito, registro auditable de opt-in, baja accesible y supresión de rebotes/quejas.

El sitio comercial puede ser estático en el frontend; el backend debe exponer los contratos de registro, catálogo de planes, contacto si se procesa en servidor y estado del servicio que esas páginas necesiten. No acoples el backend al contenido visual o CMS no requerido.

## 10. Planes, suscripciones, facturación y créditos

Diseña un dominio comercial conectado a un proveedor de pagos mediante adaptadores verificables y configuración externa:

- Catálogo de planes con moneda, periodo, precios, cuotas, límites, capacidades incluidas y disponibilidad.
- Suscripciones por espacio de trabajo, pruebas, cambios de plan, renovación, cancelación al cierre o inmediata, reactivación y estados pendientes.
- Historial de facturas, pagos, reembolsos y fallos; referencias a documentos emitidos por el proveedor, sin almacenar datos de tarjeta.
- Webhooks firmados, deduplicados e idempotentes; conciliación de eventos fuera de orden y reintentos seguros.
- Libro mayor inmutable de créditos para generación: concesiones, compras, reservas, consumos, expiraciones, ajustes y reversos con referencia idempotente a cada operación/intento. El cliente ve y usa créditos; el backend conserva por separado importes monetarios, tarifa del proveedor, consumo medido, costo atribuible y precio final. Reserva el máximo estimado antes de llamar al proveedor; liquida el consumo confirmado y libera el saldo restante. Si la respuesta del proveedor deja el costo incierto, conserva la reserva hasta conciliar. Un reintento de transporte no duplica un asiento; un nuevo intento de inferencia sí registra separadamente cualquier costo adicional que cobre el proveedor. Ante fallo, liquida solo consumo realmente reportado/conciliado; un crédito promocional o reembolso comercial se registra como movimiento distinto del costo proveedor. La conversión de precio calculado a créditos y los paquetes siguen la política versionada de [`docs/decision-comercial-v1.md`](docs/decision-comercial-v1.md), con valor de referencia inicial de USD 0,01 por crédito; los valores de planes/paquetes son hipótesis y requieren simulación financiera antes de publicarse.
- Aplica una meta inicial de 30 % de margen operativo antes del impuesto a la renta, si el catálogo incluye costos asignados completos. Para costo Cloudflare `C`, operación asignada `O`, cargo fijo PSP asignado `F`, comisión porcentual `r` y margen `m`, usa `P=(C+O+F)/(1-r-m)`. Distingue margen bruto de inferencia, contribución tras comisiones, margen operativo tras asignaciones y resultado neto; no prometas 30 % neto si faltan costos. Conserva tarifa, unidad, moneda, fuente, vigencia, costos, comisión, porcentaje de margen y precio aplicado por operación.
- El almacenamiento persistente R2 es opcional y requiere opt-in y un saldo monetario independiente, recargable y no intercambiable con créditos de generación. Factura el uso atribuible por GB-mes y por operaciones R2 billables/retrieval cuando apliquen, basado en tarifas oficiales versionadas. Aplica a R2 la misma meta operativa inicial del 30 % sobre el precio final del costo y operación asignados, incluyendo comisión PSP contabilizada una sola vez, y conserva su ledger/estado estimado/conciliado separados de IA. Aplica la recarga, gracia, ventana de exportación y retención temporal definidas en [`docs/decision-comercial-v1.md`](docs/decision-comercial-v1.md); el uso temporal no implica guardar permanentemente un archivo.
- Usa tarifas oficiales versionadas del [catálogo Cloudflare AI](https://developers.cloudflare.com/ai/models/) para modelos de terceros y de [R2](https://developers.cloudflare.com/r2/pricing/), sin fijar precios actuales en código ni en el roadmap. Workers AI solo aplica a modelos `@cf/...` habilitados explícitamente. Registra el consumo en las unidades oficiales por modelo y en GB-mes para almacenamiento, y conserva fuente/fecha de la tarifa.
- Modela el costo operativo de XOVYRA, costo base Cloudflare estimado/conciliado, margen de contribución, precio comercial, créditos de generación y saldo de almacenamiento como valores/libros distintos. La cuenta Cloudflare puede agregar tenants y aplicar free tiers/créditos globales; calcula el precio del cliente desde las tarifas unitarias oficiales versionadas antes de beneficios de cuenta y concilia diferencias de factura global por separado. Informa claramente unidad, consumo, cálculo y periodo cobrados.
- Cuotas por periodo, usuario, modalidad, proyecto y proveedor, con reglas de aplicación y zona horaria explícitas.
- Reserva/liberación/conciliación de créditos para tareas asíncronas: define comportamiento ante cancelación, fallo, incertidumbre y coste real tardío.
- API para saldo, consumo, previsiones, límites efectivos, transacciones, suscripción y factura; separar estimación de cargos y consumo confirmado.
- Política explícita para sobrepasos, agotamiento, gracia, suspensión y restauración; los workers deben verificar y hacer cumplir límites antes y durante las operaciones costosas.
- Auditoría de cambios comerciales y protección contra doble cobro/doble consumo mediante claves idempotentes.

No inventes precios ni confirmes un pago por una respuesta del frontend. Los importes finales y estados comerciales proceden del proveedor y del backend, con conciliación documentada.

## 11. Proyectos multimodales y capacidades de proveedores

Generaliza el modelo de contenido para que un proyecto pueda ser de imagen, video, clip, audio, voz, música, texto o modalidad combinada, sin exigir el flujo financiero/editorial completo.

- Define tipos de proyecto, tareas y activos extensibles; conserva relaciones y versiones entre entradas, generación y resultados.
- Representa flujos especializados (brief, investigación, guion, storyboard, render, aprobación) como workflows opcionales y componibles.
- Implementa un registro de capacidades reportadas/validadas por proveedor y modelo: modalidades de entrada/salida, parámetros, formatos, límites, restricciones y versión/fecha de verificación.
- Expón esquemas tipados de parámetros y validación por capacidad; rechaza parámetros desconocidos o incompatibles antes de llamar al proveedor.
- Permite consultar capacidades disponibles y alternativas compatibles; no infieras soporte por nombre del modelo.
- Registra la versión del esquema/capacidad usada en cada operación para poder reproducir y auditar resultados.
- Gestiona disponibilidad, límites, precios conocidos, latencia y errores como datos con origen y vigencia; no presentes estimaciones caducadas como garantía.
- Los adaptadores deben soportar capacidades opcionales de manera explícita (asíncrono, cancelación, webhook, idempotencia, archivos grandes) según contratos oficiales.

## 12. Equipos, integraciones y proveedores por espacio

Implementa la colaboración y conexión de servicios como capacidades con alcance y permisos definidos:

- Invitaciones por correo con token de un solo uso, expiración, revocación, aceptación idempotente y límites contra abuso.
- Membresías, roles personalizados y permisos de espacio; asignación de tareas y autoría de comentarios/actividad cuando formen parte de los dominios de contenido.
- Proveedores de IA e integraciones conectados a nivel de espacio, salvo los servicios administrados globalmente por plataforma.
- Modelo de credenciales que distinga credenciales propias del cliente y credenciales administradas por plataforma; cifrado con claves externas, rotación, alcance mínimo y nunca devolver secretos.
- Descubrimiento de integraciones disponibles, conexión, prueba, reconexión y desconexión; reportar capacidades, estado y errores normalizados.
- OAuth con `state`, PKCE cuando aplique, validación de redirect URI, protección CSRF, revocación y cifrado de tokens.
- Eventos de webhook autenticados, límites de consumo, cuotas, errores y auditoría asociados al espacio correcto.

## 13. Gestión de usuarios y canales

Implementa:

- CRUD de usuarios.
- Invitaciones.
- Activación y desactivación.
- Asignación de roles.
- Historial de actividad.
- Preferencias.
- Zona horaria.
- Configuración de notificaciones.
- Administración de canales.
- Vinculación OAuth con YouTube.
- Estado de conexión.
- Renovación y revocación de credenciales.
- Identificadores de canal.
- Nombre, descripción y metadatos del canal.
- Configuración de idioma y audiencia.
- Categoría y estilo editorial.
- Reglas de marca.
- Plantillas de publicación.
- Configuración de privacidad predeterminada.
- Límites y cuotas.
- Desconexión segura.

Cifra las credenciales sensibles en reposo usando un mecanismo de gestión de claves configurado fuera de la base de datos. Nunca registres tokens completos.

## 14. Gestión editorial y proyectos

Implementa proyectos y series de contenido como dominios generales. Deben admitir contenido multimodal y workflows opcionales; conserva los atributos editoriales financieros como configuración/plantilla especializada.

Cada proyecto debe contemplar, según corresponda:

- Título.
- Identificador.
- Descripción.
- Canal asociado.
- Tema financiero.
- Objetivo editorial.
- Público objetivo.
- País o mercado.
- Idioma.
- Duración objetivo.
- Formato: Short o largo.
- Fecha de inicio y fin.
- Frecuencia.
- Plantilla.
- Estado.
- Espacio de trabajo propietario y tipo/modalidad de proyecto.
- Metadatos extensibles validados por esquema y versionados; no uses JSON arbitrario como sustituto de contratos.
- Responsable.
- Presupuesto.
- Coste acumulado.
- Etiquetas.
- Palabras clave.
- Reglas de fuentes.
- Reglas de estilo.
- Restricciones de contenido.
- Configuración audiovisual.
- Configuración de narración.
- Configuración de subtítulos.
- Historial de versiones.

Implementa CRUD completo, archivado, duplicación, búsqueda, filtros y auditoría.

## 15. Investigación financiera y fuentes

Desarrolla un módulo de investigación que pueda utilizar fuentes externas mediante conectores configurables.

Debe permitir:

- Crear temas de investigación.
- Buscar información a través de proveedores autorizados.
- Importar enlaces y documentos.
- Registrar publicaciones, autores, fechas y entidades.
- Extraer texto de documentos compatibles.
- Asociar fuentes con afirmaciones concretas.
- Detectar posibles duplicados.
- Guardar citas y fragmentos de respaldo.
- Registrar la fecha de consulta.
- Evaluar actualidad y calidad de las fuentes.
- Comparar afirmaciones contradictorias.
- Señalar cifras sin respaldo.
- Mantener trazabilidad desde el guion hasta sus fuentes.
- Generar una bibliografía para cada video.
- Volver a comprobar datos antes de aprobar un guion.

Prioriza fuentes primarias: organismos oficiales, bancos centrales, estadísticas públicas, informes corporativos y documentación original.

La IA no debe inventar citas, URLs, cifras ni documentos. Si no se encuentra evidencia suficiente, la afirmación deberá quedar marcada como no verificada.

Para contenidos financieros, registra:

- Cifra.
- Moneda.
- Unidad.
- Fecha de referencia.
- Periodo.
- País o mercado.
- Fuente.
- Método de cálculo, cuando corresponda.
- Nivel de verificación.
- Riesgo de interpretación engañosa.

Distingue datos históricos de datos actuales y proyecciones de resultados comprobados.

## 16. Generación y administración de guiones

Implementa un ciclo completo de guion:

1. Brief editorial.
2. Investigación.
3. Estructura narrativa.
4. Primer borrador.
5. Revisión factual.
6. Revisión editorial.
7. Adaptación al formato.
8. Optimización de duración.
9. Título provisional.
10. Descripción.
11. Llamada a la acción, cuando sea apropiada.
12. Subtítulos o transcripción.
13. Versión para revisión.
14. Aprobación o solicitud de cambios.

Los guiones deben soportar:

- Edición manual.
- Versiones inmutables de revisiones anteriores.
- Comparación de versiones.
- Comentarios.
- Etiquetas.
- Estado editorial.
- Fuentes vinculadas por afirmación.
- Duración estimada.
- Recuento de palabras.
- Notas de pronunciación.
- Instrucciones visuales por escena.
- Instrucciones de narración.
- Marcadores de pausa.
- Referencias a imágenes, gráficos y clips.
- Registro de cambios.
- Rechazo con motivo.
- Nueva revisión y aprobación.

No sobrescribas una versión que ya fue aprobada. Una modificación sustancial debe producir una versión nueva y requerir una nueva aprobación.

## 17. Sistema de proveedores de IA externos

No instales ni ejecutes modelos de IA locales. Todo servicio de generación debe consumirse mediante APIs externas.

Workers AI y Cloudflare AI/AI Gateway son canales distintos del catálogo inicial. Implementa los 10 IDs Workers AI `@cf/...` seleccionados y los modelos terceros `black-forest-labs/flux-3-image-klein`, `pruna/p-image`, `pruna/p-video` y `minimax/h3-max`, según catálogo, capacidades y política comercial verificados. Workers AI usa su billing por neurons con unidades/precios oficiales por modelo y su cuota diaria global; AI Gateway para modelos terceros usa Unified Billing o BYOK y conciliación propia. No mezcles unidades ni atribuyas cuotas globales Cloudflare a tenants como costo unitario. Mantén credenciales solo en servidor/gestor de secretos, precios versionados por modelo/unidad, estados Beta y términos/licencias; verifica capacidades/parámetros antes de consumir y no simules generación.

Crea interfaces intercambiables, por ejemplo:

- `TextProvider`
- `ResearchProvider`
- `ImageProvider`
- `VideoProvider`
- `TtsProvider`
- `TranscriptionProvider`
- `ModerationProvider`

Cada integración debe tener:

- Cliente HTTP.
- Autenticación segura.
- Configuración de endpoint.
- Modelo o servicio.
- Límites de solicitudes.
- Timeouts.
- Reintentos controlados.
- Backoff exponencial con jitter.
- Clasificación de errores.
- Identificadores de solicitud.
- Registro de consumo.
- Medición de latencia.
- Coste estimado y coste real cuando esté disponible.
- Cancelación cuando sea compatible.
- Consulta del estado de trabajos asíncronos.
- Manejo de webhooks autenticados cuando corresponda.
- Pruebas unitarias y pruebas de contrato.
- Capacidad de cambiar de proveedor.

No asumas que todos los proveedores admiten generación asíncrona, webhooks, cancelación o idempotencia. Implementa cada capacidad según su documentación real.

Permite configurar modelos y capacidades de Cloudflare por tarea. Proveedores alternativos solo se habilitan mediante implementación y credenciales aprobadas explícitamente; el cambio debe respetar compatibilidad, calidad, privacidad y presupuesto.

No cambies automáticamente a un proveedor más costoso sin que lo permita una política explícita.

## 18. Motor multimodal, composición y producción audiovisual

Separa el motor general de operaciones multimodales de los workflows especializados de producción editorial. Una operación directa no requiere crear proyecto editorial, canal, brief, investigación, guion, storyboard ni aprobación de publicación.

### Operaciones independientes

Expón operaciones tipadas para texto, imágenes, clips/video, voz/narración, audio/música y transcripción/moderación cuando el catálogo Cloudflare AI/AI Gateway u otra integración habilitada explícitamente lo soporte. Cada operación acepta únicamente las entradas/parámetros de su capacidad verificada; valida permisos, cuota y reserva de créditos antes de ejecutar; registra modelo, versión de capacidad, unidad consumida, estimación, ejecución, resultado y errores; y crea un activo versionado bajo el tenant/propietario autorizado.

Un recurso resultante puede existir sin proyecto editorial y posteriormente asociarse a un proyecto, composición o workflow. Operaciones síncronas y asíncronas reportan estados reales e idempotencia cuando esté disponible; no fuerces el paso por un workflow de video.

### Edición y composición

Permite componer o transformar activos existentes (pistas de audio, imágenes, clips, texto, subtítulos, escenas) en tareas independientes y versionadas. Valida formatos y capacidades, conserva los originales, registra dependencias y produce nuevos activos en R2.

### Workflow editorial opcional

Conserva la secuencia especializada de investigación, guion, escenas, generación de recursos, render, revisión humana y publicación como workflow seleccionable. No es requisito del motor general.

### Producción audiovisual

Implementa el proceso integral para crear videos a partir de guiones aprobados para producción, como workflow especializado.

### Imágenes y clips

- Generar imágenes mediante APIs externas.
- Generar clips de video mediante APIs externas.
- Definir prompts por escena.
- Mantener coherencia visual.
- Relacionar cada recurso con su escena.
- Registrar proveedor, modelo, parámetros y coste.
- Guardar los resultados de forma privada en R2 mediante el servicio de almacenamiento autorizado.
- Validar resolución, duración, códec y formato.
- Detectar fallos y recursos incompletos.
- Permitir regenerar una escena sin destruir versiones anteriores.
- Permitir sustituir manualmente recursos.
- Evitar duplicaciones innecesarias.
- Respetar límites y políticas de cada proveedor.

No intentes generar toda la duración de un video mediante una única solicitud si el proveedor no lo admite. Construye una secuencia de escenas y clips conforme a las capacidades reales de cada servicio.

### Narración

Implementa:

- Generación de voz mediante TTS externo.
- Selección de voz e idioma.
- Configuración de velocidad y tono cuando el proveedor lo permita.
- Pronunciación de cifras, símbolos, nombres y siglas.
- División de narración en fragmentos cuando resulte necesario.
- Regeneración por fragmento.
- Normalización de audio.
- Control de volumen y silencios.
- Registro de duración y coste.

### Música y efectos

Incluye gestión de pistas musicales y efectos con licencias verificables.

Registra origen, licencia, restricciones de uso y atribución requerida. No utilices recursos con licencia desconocida como si estuvieran autorizados para monetización.

### Subtítulos

Implementa:

- Transcripción.
- Generación de subtítulos.
- Formato SRT y VTT.
- Sincronización temporal.
- Corrección manual.
- Validación de caracteres y tiempos.
- Subtítulos incrustados opcionales.
- Versionado de archivos.

### Renderizado con FFmpeg

Construye un servicio de renderizado real con FFmpeg y FFprobe.

Debe soportar:

- Video vertical 9:16.
- Video horizontal 16:9.
- Resoluciones configurables.
- FPS configurables.
- Concatenación de clips.
- Imágenes con movimiento y transiciones.
- Gráficos financieros.
- Texto en pantalla.
- Subtítulos.
- Narración.
- Música.
- Mezcla de audio.
- Ducking de música bajo la voz cuando corresponda.
- Ajustes de volumen.
- Conversión de formatos.
- Miniaturas.
- Codificación configurable.
- Normalización audiovisual.
- Validación de duración.
- Validación del archivo final.
- Logs de FFmpeg.
- Cancelación y recuperación de trabajos.
- Uso limitado de CPU y memoria.

No ejecutes comandos del sistema construidos con interpolación de cadenas sin control. Usa invocaciones seguras de procesos con argumentos separados, límites de recursos y validación de rutas.

Conserva el proyecto de edición y los metadatos necesarios para reproducir un render.

## 19. Control de calidad automático

Implementa validaciones previas a la revisión humana.

Comprueba:

- Existencia de todas las escenas requeridas.
- Integridad de imágenes, clips y audio.
- Duración.
- Resolución.
- Relación de aspecto.
- Sincronización de audio.
- Ausencia de archivos vacíos.
- Subtítulos.
- Niveles de audio.
- Posibles pantallas negras.
- Clips congelados.
- Desbordamiento de textos.
- Coherencia entre guion y escenas cuando se disponga de una herramienta adecuada.
- Calidad mínima de narración.
- Fuentes y cifras verificadas.
- Derechos y licencias de recursos.
- Título y descripción.
- Miniatura.
- Posibles afirmaciones financieras engañosas.
- Duplicación de contenido.
- Presupuesto.
- Reglas editoriales y políticas de la plataforma.

Los controles automatizados no deben declararse infalibles. Cuando no se pueda verificar una condición, registra el resultado como desconocido o pendiente, no como aprobado.

Permite umbrales configurables y registra cada resultado con su versión.

## 20. Motor de trabajos y orquestación

Implementa una cola persistente y un motor de workflows con tareas dependientes.

Cada trabajo debe incluir:

- ID.
- Tipo.
- Proyecto.
- Contenido.
- Prioridad.
- Estado.
- Payload validado.
- Fecha de creación.
- Fecha de inicio.
- Fecha de finalización.
- Intentos.
- Próximo intento.
- Worker asignado.
- Heartbeat o lease.
- Error estructurado.
- Coste.
- Tiempo de ejecución.
- Dependencias.
- Clave de idempotencia.
- Cancelación.
- Historial de eventos.

Estados mínimos:

- Pendiente.
- Preparado.
- En ejecución.
- Esperando proveedor externo.
- Reintentando.
- Completado.
- Fallido.
- Cancelado.
- En revisión.
- Bloqueado por aprobación.

Implementa:

- Reintentos diferenciados por tipo de error.
- Backoff exponencial.
- Recuperación de trabajos abandonados.
- Leases para evitar ejecución simultánea duplicada.
- Límites globales y por proveedor.
- Límites por canal y proyecto.
- Priorización.
- Cancelación segura.
- Pausa y reanudación.
- Dependencias entre tareas.
- Dead-letter o equivalente para trabajos agotados.
- Reprocesamiento manual.
- Detección de tareas bloqueadas.
- Recuperación tras reinicio.
- Métricas de cola y latencia.

Evita duplicar cargos externos por reintentos. Utiliza idempotencia cuando el proveedor lo soporte y conserva identificadores externos.

## 21. Flujo editorial y aprobación humana obligatoria

Este requisito es crítico.

Ningún contenido puede publicarse sin una aprobación humana explícita, vigente y vinculada a la versión exacta que se va a publicar.

El flujo debe contemplar:

Borrador → Investigación → Guion → Producción → Renderizado → Control de calidad → Revisión humana → Aprobado → Programado o listo para publicar → Publicado.

También debe admitir:

- Rechazado.
- Requiere cambios.
- Bloqueado.
- Cancelado.
- Fallido.
- Archivado.

Antes de aprobar, el revisor debe poder acceder a:

- Video final.
- Miniatura.
- Guion.
- Fuentes.
- Datos y cifras.
- Título.
- Descripción.
- Etiquetas.
- Subtítulos.
- Costes.
- Resultados del control de calidad.
- Historial de cambios.

La aprobación debe registrar:

- Usuario.
- Fecha y hora.
- Versión aprobada.
- Hash del video final.
- Hash de la miniatura.
- Metadatos aprobados.
- Decisión.
- Comentarios.

**Si cambia el video, la miniatura, el guion relevante, el título, la descripción u otro elemento sometido a aprobación, invalida la aprobación cuando el cambio afecte al contenido aprobado. Exige una nueva revisión.**

Una vez aprobado, congela una instantánea inmutable de los artefactos y metadatos que se publicarán.

La publicación debe verificar la vigencia de la aprobación justo antes de ejecutar la llamada externa.

No permitas que un job automático, un reintento o una tarea programada eluda este control.

## 22. Publicación en YouTube

Implementa la integración real mediante la YouTube Data API oficial y OAuth 2.0.

Incluye:

- Conexión y desconexión del canal.
- Gestión segura de tokens.
- Validación del canal de destino.
- Subida de videos.
- Gestión de cargas reanudables cuando corresponda.
- Seguimiento del progreso.
- Recuperación ante interrupciones.
- Título.
- Descripción.
- Etiquetas.
- Categoría.
- Idioma.
- Miniatura.
- Subtítulos cuando la API lo permita.
- Visibilidad.
- Fecha programada.
- Identificador del video publicado.
- URL.
- Estado remoto.
- Errores de publicación.
- Reintentos seguros.
- Reconciliación entre estado local y estado remoto.
- Registro de cuotas de API.
- Gestión de límites de la plataforma.

No asumas que todos los proyectos de Google tienen acceso inmediato a todas las capacidades. Verifica los requisitos y las restricciones de la API vigente.

Distingue la publicación privada, no listada y pública. Configura el valor predeterminado más seguro.

La publicación pública o programada debe requerir aprobación humana válida. Una publicación privada de prueba también debe estar sujeta a las políticas del entorno.

Implementa reconciliación para detectar si una carga se completó externamente aunque el servidor haya perdido la respuesta.

## 23. Programación y calendario

Implementa un planificador con:

- Calendario editorial.
- Fecha y hora.
- Zona horaria IANA.
- Frecuencia diaria.
- Días de publicación.
- Ventanas horarias.
- Restricciones por canal.
- Capacidad de workers.
- Presupuesto disponible.
- Estados de aprobación.
- Reprogramación.
- Cancelación.
- Detección de conflictos.
- Prevención de publicaciones duplicadas.
- Manejo correcto de cambios de horario.
- Reintentos y alertas.

La programación no equivale a permiso para publicar. El planificador debe detener o aplazar cualquier contenido sin aprobación válida.

## 24. Costes, créditos y presupuestos

Crea un módulo de costes con registro por operación.

Separa el coste del proveedor (costo operativo de XOVYRA), el consumo imputado al cliente y los créditos/importe facturado. Relaciona las operaciones con el ledger de créditos cuando aplique, pero conserva registros distintos y conciliables. No dupliques ni derives un cargo comercial directamente de una estimación de coste.

Registra:

- Proveedor.
- Modelo.
- Tipo de operación.
- Proyecto.
- Contenido.
- Trabajo.
- Unidades consumidas.
- Duración generada.
- Tokens, imágenes o segundos, cuando corresponda.
- Precio unitario conocido.
- Coste estimado.
- Coste real comunicado por el proveedor.
- Moneda.
- Fecha.
- Estado de conciliación.

No confundas costes estimados con costes reales.

Permite:

- Presupuesto diario.
- Presupuesto mensual.
- Presupuesto por proyecto.
- Presupuesto por video.
- Límites por proveedor.
- Alertas de consumo.
- Proyecciones.
- Comparación entre coste previsto y real.
- Historial de precios.
- Reglas de detención por presupuesto.

Antes de iniciar una tarea costosa, comprueba los límites disponibles. Para operaciones con coste incierto, registra la incertidumbre y aplica un límite de exposición.

## 25. Analítica de YouTube

Implementa la integración con la YouTube Analytics API cuando el acceso y los permisos estén disponibles.

Contempla:

- Visualizaciones.
- Tiempo de reproducción.
- Duración media.
- Impresiones cuando estén disponibles.
- CTR cuando esté disponible.
- Suscriptores ganados.
- Ingresos cuando la cuenta tenga acceso a esos datos.
- Comparación entre videos.
- Rendimiento de Shorts frente a videos largos.
- Periodos de tiempo.
- Evolución histórica.
- Etiquetas y categorías.
- Informes exportables.

Registra la procedencia y fecha de actualización de las métricas.

No inventes ingresos ni rellenes datos ausentes con estimaciones presentadas como valores reales. Separa explícitamente las proyecciones de las métricas oficiales.

## 26. Notificaciones

Implementa un sistema de notificaciones internas para:

- Video listo para revisar.
- Aprobación pendiente.
- Rechazo.
- Error de generación.
- Error de renderizado.
- Error de publicación.
- Presupuesto próximo al límite.
- Disco próximo a llenarse.
- Token OAuth caducado.
- Integración desconectada.
- Cola bloqueada.
- Copia de seguridad fallida.
- Publicación completada.

Diseña un sistema extensible para correo u otros canales de notificación configurables.

No afirmes que se ha enviado un correo si el proveedor de correo no está configurado o ha fallado.

## 27. Auditoría y observabilidad

Implementa:

- Logs estructurados.
- Correlation ID.
- Request ID.
- IDs de trabajo y contenido.
- Métricas de rendimiento.
- Duración de solicitudes.
- Errores por endpoint.
- Estado de workers.
- Longitud de colas.
- Uso de disco.
- Uso de memoria y CPU.
- Consumo de APIs.
- Costes.
- Auditoría de operaciones críticas.
- Historial de cambios.
- Alertas operativas.

Todos los dominios deben emitir errores estructurados y diagnosticables, no solo un tipo de error genérico. Los eventos internos incluyen bounded context, operación, dependencia, clase/código de error, resultado, `correlation_id` y `trace_id`; añaden SQLSTATE/gRPC/HTTP/proveedor y constraint/tabla/columna cuando esos datos estén disponibles y sean seguros. Diferencia validación, autorización, conflicto, timeout, conectividad/pool, persistencia, serialización y fallo externo. Nunca registres secretos, cookies, tokens, URLs firmadas, contenido, valores de entrada sensibles o detalles de dependencias que puedan incluirlos.

Las respuestas REST usan un contrato estable con `code`, `message` y `details` seguros, más header `X-Correlation-ID`; `details` describe categoría, operación segura, posibilidad/tiempo de reintento o campos que el usuario puede corregir. Los errores de campo incluyen rutas/códigos de validación saneados cuando corresponda. No exponen SQL, stack traces, secretos ni mensajes crudos inseguros de infraestructura. Los frontends presentan el mensaje claro, la referencia de solicitud y la recuperación aplicable. Las pruebas de contrato verifican el mapeo módulo → gRPC → BFF → REST → UI y la ausencia de datos sensibles.

Nunca registres contraseñas, cookies, tokens, secretos o datos personales innecesarios.

Los eventos de auditoría deben registrar usuario, acción, recurso, resultado, fecha y contexto relevante.

## 28. API REST

Diseña una API REST versionada, por ejemplo `/api/v1`.

Implementa endpoints reales para todos los módulos definidos: autenticación y registro, usuarios, espacios de trabajo, membresías, invitaciones, permisos, catálogo de planes, suscripciones, facturación, créditos/cuotas, integraciones, Cloudflare AI/AI Gateway y capacidades, proyectos multimodales, canales, investigación, fuentes, guiones, escenas, recursos, renders, trabajos, revisión, aprobaciones, publicación, calendario, analítica, costes, configuración, R2, notificaciones y auditoría.

El plano de control de archivos debe incluir intención/autorización de carga, renovación limitada, finalización, verificación, estado, metadatos, cuota/consumo, compartir, borrado y emisión de URL temporal de descarga. El API nunca recibe ni retransmite el cuerpo binario. Registra y autoriza cada operación contra tenant, propietario y ACL; no permite listar o leer por prefijo arbitrario.

Define contratos del plano de cliente bajo la API versionada y contratos de control de plataforma separados bajo `/platform/*`, con autenticación y autorización independientes. No mezcles navegación ni autoridad entre ambos ámbitos. Toda ruta cliente que opere sobre un espacio debe resolver y autorizar la membresía/contexto del tenant en el servidor.

Incluye endpoints públicos mínimos para registro, verificación, catálogo comercial cuando corresponda y estado del servicio. No expongas por endpoints públicos detalles operativos internos ni información de clientes.

Los contratos deben describir límites y estados de suscripción, crédito, consumo, capacidades dinámicas, integraciones, errores de dominio e idempotencia. Publica OpenAPI y pruebas de contrato para que el frontend pueda generar tipos y formularios sin inferir capacidades.

Define:

- Esquemas de petición y respuesta.
- Validación.
- Códigos HTTP coherentes.
- Paginación.
- Ordenamiento.
- Filtros.
- Búsqueda.
- Límites.
- Manejo uniforme de errores.
- Idempotencia para operaciones críticas.
- Documentación OpenAPI.
- Versionado y compatibilidad.

Publica contratos completos y coherentes para: registro/verificación/login/sesión; organización activa y membresía; recursos con alcance/propietario/ACL; intención, finalización y descarga directa de objetos R2; operaciones multimodales independientes; estados de trabajos, progreso opcional, errores tipados y reintentos; capacidades por modelo; estimaciones y liquidaciones de créditos; uso/cargos de IA y almacenamiento; y facturación. Incluye ejemplos de errores de autorización, cuota agotada, límite del proveedor, URL de transferencia expirada, carga en validación y pago pendiente.

Usa una envoltura de error estable con código de dominio, mensaje seguro, detalles de validación y correlation/request ID. Distingue progreso desconocido de cero progreso. Añade pruebas de contrato OpenAPI y garantiza que clientes generados puedan representar estados evolutivos sin perder compatibilidad.

Usa el sistema realtime transversal WebSocket + TanStack Query para cambios de datos, notificaciones y progreso en tiempo real. REST conserva lecturas, comandos y recuperación cursor/snapshot. No abras canales o sockets independientes por pantalla salvo una decisión arquitectónica versionada. Implementa autenticación, permisos, aislamiento tenant/plataforma, revocación, reconexión, deduplicación y recuperación en todos los canales.

No devuelvas estructuras internas de Rust, trazas de errores ni secretos al cliente.

## 29. Modelo de datos y migraciones

Diseña schemas PostgreSQL normalizados, con relaciones e índices adecuados, dentro de una única base de datos física compartida por todos los módulos. Define explícitamente el schema y ownership de cada tabla para cada módulo; la base compartida no autoriza consultas o escrituras cruzadas.

Incluye entidades para:

- Usuarios.
- Espacios de trabajo/tenants y estado.
- Membresías, invitaciones y su ciclo de vida.
- Roles y permisos con ámbito de plataforma o espacio.
- Registro de onboarding y verificación de correo.
- Planes/versiones, suscripciones, periodos comerciales y estados.
- Clientes de facturación, eventos de webhook, facturas, pagos y conciliaciones.
- Libro mayor de créditos, reservas, consumos, expiraciones, ajustes y reversos.
- Cuotas y políticas de consumo.
- Tarifas Cloudflare versionadas por modelo/unidad, tarifas R2 por clase/unidad, margen comercial, vigencias y fuente oficial.
- Transacciones de créditos con reserva/liquidación/devolución y precios cliente aplicados.
- Libro de uso de IA por tenant/modelo/unidad y libro de uso R2 por objeto/propietario/GB-mes.
- Incidencias/operaciones de soporte de plataforma y concesiones temporales de acceso auditadas.
- Roles.
- Permisos.
- Sesiones.
- Tokens de recuperación.
- Canales.
- Credenciales OAuth.
- Proyectos.
- Series.
- Briefs.
- Investigaciones.
- Fuentes.
- Afirmaciones y cifras.
- Guiones y versiones.
- Escenas.
- Recursos multimedia.
- Archivos.
- Objetos R2, claves internas opacas, estado de validación, checksum, versión, almacenamiento de origen y ciclo de vida.
- Intenciones de carga, operaciones multipart, expiraciones, partes y resultados de validación.
- Uso de almacenamiento por tenant/propietario/objeto y asientos de ledger de uso/facturación.
- Reservas y conciliaciones de cuota de almacenamiento.
- Comparticiones ACL explícitas y eventos de acceso/descarga.
- Proveedores.
- Integraciones por espacio y referencias cifradas a credenciales.
- Modelos y declaraciones/versiones verificadas de capacidades.
- Modelos de IA.
- Solicitudes externas.
- Registros de uso de almacenamiento y cargos atribuibles al propietario/espacio.
- Autorizaciones temporales y sesiones de carga multipart (sin persistir URLs firmadas completas).
- Eventos de validación, cuarentena, acceso, compartición y eliminación de objetos.
- Trabajos.
- Dependencias de trabajos.
- Eventos.
- Renders.
- Subtítulos.
- Miniaturas.
- Revisiones.
- Aprobaciones.
- Publicaciones.
- Programaciones.
- Métricas.
- Costes.
- Presupuestos.
- Notificaciones.
- Configuraciones.
- Auditorías.
- Copias de seguridad.

Añade claves foráneas, restricciones, índices, unicidad e integridad referencial dentro de los límites de propiedad del módulo. No uses FK ni JOINs entre schemas de distintos módulos como mecanismo de integración; usa contratos gRPC/eventos y proyecciones locales cuando corresponda.

Toda tabla de datos de cliente debe tener una relación inequívoca con su espacio de trabajo o estar claramente identificada como dato global/plataforma. Define índices y claves únicas con tenant cuando corresponda, y pruebas de migración/integridad que detecten relaciones cruzadas. Aplica retención y anonimización apropiadas a datos de facturación y auditoría.

Implementa migraciones versionadas independientes dentro de cada módulo, aplicadas coordinadamente contra la misma base PostgreSQL. Cada conjunto solo puede crear/cambiar los objetos cuyo ownership pertenece a ese módulo. No borres datos históricos en migraciones sin un procedimiento explícito; prueba el orden de migración desde una base vacía y en upgrades.

Inicializa los roles y permisos de sistema requeridos para operar mediante migraciones versionadas del módulo dueño. No incluyas datasets de ejemplo, datos de demostración ni credenciales/contraseñas en migraciones o carga inicial.

## 30. Seguridad adicional

Implementa:

- Rate limiting.
- Límites de payload.
- Validación de tipos de archivo.
- Prevención de SSRF en funciones que importen URLs.
- Protección contra inyección SQL.
- Protección contra path traversal.
- Restricción de comandos externos.
- Verificación de firmas de webhooks.
- Límites de timeout.
- Cifrado de credenciales.
- Política de secretos.
- Cabeceras de seguridad.
- CORS restringido.
- Revisión de dependencias.
- Protección de datos sensibles.
- Control de acceso a archivos.
- Políticas de retención.
- Auditoría.
- Manejo seguro de errores.
- Reglas de backup y restauración.

Si se admiten URLs externas, bloquea direcciones internas, loopback, link-local y otros destinos no autorizados para prevenir SSRF. Revalida las direcciones después de redirecciones.

## 31. Configuración y despliegue

Entrega:

- `Cargo.toml` organizado.
- Configuración por entornos.
- Archivo `.env.example` sin secretos reales.
- Migraciones SQL.
- Dockerfile si aporta valor.
- `docker-compose.yml` para desarrollo y despliegue local cuando resulte adecuado.
- Configuración de PostgreSQL.
- Configuración de workers.
- Configuración de FFmpeg.
- Configuración de Cloudflare AI/AI Gateway, Cloudflare R2, CORS, credenciales server-side y límites de transferencia.
- Servicio de ejecución en Linux, por ejemplo systemd, si corresponde.
- Proxy inverso y HTTPS.
- Scripts de instalación.
- Scripts de actualización.
- Scripts de backup.
- Scripts de restauración.
- Procedimientos de recuperación.
- Documentación de variables de entorno.

No expongas PostgreSQL directamente a Internet. Aplica permisos mínimos y separa credenciales por entorno.

El sistema debe arrancar de forma reproducible y recuperar los trabajos pendientes tras un reinicio.

## 32. Estrategia completa de pruebas

Implementa:

- Pruebas unitarias.
- Pruebas de integración con PostgreSQL.
- Pruebas de migraciones.
- Pruebas de autenticación.
- Pruebas de autorización por rol.
- Pruebas de almacenamiento.
- Pruebas de autorización de carga/descarga directa R2, caducidad de URLs firmadas, CORS, aislamiento de objetos, multipart abandonado y conciliación de cuota/uso.
- Pruebas de recuperación de trabajos.
- Pruebas de idempotencia.
- Pruebas de proveedores contra sus entornos oficiales de prueba/sandbox mediante adapters reales.
- Pruebas de contrato para APIs externas.
- Pruebas de renderizado con archivos generados de forma transitoria mediante el flujo real de procesamiento, sin fixtures ni archivos de muestra prefabricados.
- Pruebas de publicación en entorno controlado cuando sea posible.
- Pruebas de regresión.
- Pruebas de concurrencia.
- Pruebas de límites de recursos.
- Pruebas de restauración de backups.
- Pruebas de seguridad relevantes.

Incluye además pruebas automatizadas de aislamiento multi-tenant (incluidos IDOR, IDs adivinados, objetos R2, trabajos y canales en tiempo real), membresías/invitaciones, registro/verificación/onboarding, autorización independiente de plataforma, transiciones de suscripción y webhooks duplicados/fuera de orden, ledger y límites de crédito concurrentes, capacidades por modelo y validación de parámetros, y separación de credenciales de integración por espacio. Comprueba que un worker no pueda ejecutar una tarea fuera del tenant persistido en el trabajo.

Prueba expresamente que no sea posible publicar sin aprobación, que la aprobación de una versión no autorice otra y que un reintento no genere una publicación duplicada.

No uses mocks, fixtures, stubs, datos seed, ejemplos ni respuestas simuladas en las pruebas. Ejecuta el código y los adapters reales en infraestructura aislada, creando transitoriamente los registros de prueba a través de los flujos reales. Si falta acceso a un sandbox externo, registra el bloqueo y no cuentes esa integración como probada.

## 33. Fases y subfases obligatorias de implementación

Implementa por dominios completos, no por capas de endpoints o esquemas. Las fases mantienen el alcance original y ahora se dividen en subfases numeradas para que su avance y cierre sean verificables. Cada dominio debe incluir modelo y migración, reglas de negocio, API/contrato documentado, validación, autorización, aislamiento por tenant cuando aplique, estados y errores, observabilidad, pruebas automatizadas y documentación operativa/contractual.

### Regla de cierre y secuencia

- Ejecuta las subfases en el orden indicado. No comiences la fase siguiente hasta cerrar al 100 % todas las subfases de la fase actual.
- Una subfase está cerrada solo cuando cumple todos sus entregables y criterios, las verificaciones reales correspondientes pasan, la documentación está actualizada y no quedan TODO, rutas ficticias ni implementaciones parciales dentro de su alcance.
- Una fase está cerrada solo cuando todas sus subfases están cerradas y sus entregables integrados funcionan juntos. Registra una lista de cierre con evidencia verificable.
- Un bloqueo por credenciales, sandbox o servicio externo no equivale a un cierre del 100 %. Documenta el bloqueo, deja la fase abierta y no avances a la fase siguiente hasta resolverlo. Puedes continuar únicamente tareas independientes dentro de la misma fase.
- Las pruebas usan código, adapters y servicios reales en entornos aislados/sandbox; no uses mocks, fakes, stubs, fixtures, datasets seed ni respuestas simuladas. Los datos transitorios se crean y eliminan mediante flujos reales.
- La trazabilidad de cada subfase debe apuntar a las secciones funcionales y criterios de aceptación de este roadmap. La subdivisión no elimina, sustituye ni reduce ningún requisito de las secciones 1–35.
- Si una dependencia técnica obliga a cambiar el orden, documenta el motivo, conserva todas las fases/subfases y sus criterios, y no declares cerrada una fase con entregables pendientes.
- **Integración arquitectónica obligatoria por fase/subfase:** antes de escribir código, documenta e implementa en la misma entrega la ruta de cada frontend (tenant y/o administración) por HTTP/REST/OpenAPI al BFF y del BFF al bounded context mediante gRPC. Para cada subfase registra cómo aplican PostgreSQL/migraciones, Redis y NATS JetStream/Outbox: integra los componentes requeridos desde el inicio y justifica de manera concreta los que no correspondan. Redis solo puede guardar caché o estado técnico efímero con TTL; PostgreSQL conserva la verdad de negocio. Los eventos durables entre módulos usan Outbox y JetStream; las operaciones síncronas usan gRPC. No se aceptan módulos con conexiones directas del frontend a servicios internos ni dejar Redis/NATS para una refactorización posterior.

### Fase 1 — Auditoría y diseño

**Objetivo:** comprender el repositorio y convertir todos los requisitos de las secciones 1–35 en un diseño ejecutable.

- **Fase 1.1 — Inventario y diagnóstico:** inspeccionar código, configuración, dependencias, migraciones, servicios, pruebas, despliegues y documentación existentes. Registrar qué funciona, qué falta, deuda, riesgos y bloqueos sin reemplazar capacidades válidas.
- **Fase 1.2 — Mapa de requisitos y dominios:** desglosar SaaS, identidad, multi-tenancy, facturación, generación multimodal, proveedores, R2, editorial, publicación, administración, pruebas y operación. Trazar cada requisito y criterio de aceptación a su dominio y futura subfase; no dejar requisitos sin asignar.
- **Fase 1.3 — Diseño arquitectónico y de seguridad:** definir crates/módulos y ownership, BFF REST, contratos gRPC, eventos NATS/Outbox, schemas de la base PostgreSQL compartida, migraciones por módulo, límites tenant, flujos directos R2, amenazas, permisos y observabilidad conforme al documento de reglas estrictas.
- **Fase 1.4 — Plan de ejecución y contratos:** documentar dependencias, orden, decisiones técnicas justificadas, contratos iniciales, verificaciones y criterios de cierre por subfase. Identificar con claridad qué depende de credenciales o sandboxes reales.

**Cierre de fase:** inventario reproducible, mapa de trazabilidad completo, arquitectura y amenazas revisadas, decisiones documentadas y backlog ordenado sin requisitos huérfanos.

### Fase 2 — Infraestructura y base de datos

**Objetivo:** establecer la plataforma técnica modular y sus fronteras reales antes de construir dominios de producto.

El diseño, gates, registro de avance y evidencia de ejecución de esta fase se mantienen en [`docs/fase-2-infraestructura-y-base-de-datos.md`](docs/fase-2-infraestructura-y-base-de-datos.md). Su estado prevalece para declarar avance; esta referencia no sustituye ninguno de los requisitos siguientes.

- **Fase 2.1 — Workspace Rust y ejecución:** configurar toolchain fijado, workspace Cargo, crates por bounded context, binarios de BFF/workers, configuración por entorno, secretos externos, logging estructurado y chequeos de compilación/formato/lints sin supresiones.
- **Fase 2.2 — Persistencia compartida y migraciones:** provisionar una única base PostgreSQL; asignar schema y ownership de tablas por módulo; implementar migraciones versionadas separadas por módulo y un coordinador de despliegue que las aplique desde cero y durante upgrades. Verificar instalación de los 17 contextos en una base temporal vacía del PostgreSQL existente, re-ejecución idempotente y que un módulo no modifica objetos de otro; limpiar únicamente la base temporal creada por la verificación.
- **Fase 2.3 — BFF y contratos internos:** establecer API REST versionada y OpenAPI en Axum; generar contratos gRPC con tonic/prost; configurar NATS JetStream/async-nats, eventos versionados y Outbox relay con ACK de persistencia antes de marcar entrega. Validar los contratos `.proto` con Buf CLI `1.50.0` y reglas `FILE` comparadas contra `main`; los cambios incompatibles requieren una nueva versión del package protobuf y migración coordinada. El BFF no contiene negocio ni accede a DB/repositorios.
- **Fase 2.4 — Runtime y operación base:** implementar arranque, apagado ante SIGINT/SIGTERM, health/readiness dependiente de PostgreSQL, Redis y NATS, pools, límites, timeouts, correlation ID, logs estructurados y propagación W3C Trace Context HTTP→gRPC→NATS; configurar Redis con autenticación, namespace/TTL y política explícita de degradación; preparar verificaciones reales repetibles sin detener los servicios persistentes del usuario. Confirmar que `frontend/tenant/` y `frontend/admin_plataform/` son paquetes independientes y consumen REST mediante su cliente API propio, y que R2 conserva transferencia directa navegador↔R2. La especialización funcional de cada aplicación se completa en sus roadmaps frontend.

**Cierre de fase:** servicio arranca de forma reproducible; contratos y adaptadores funcionan; migraciones de todos los módulos aplican coordinadamente a la misma DB; no hay acceso cruzado; health, logs y trazas son utilizables.

### Fase 3 — Seguridad y usuarios

**Objetivo:** completar identidad, ciclo de vida de cuenta y separación de identidad cliente/operador.

El diseño, los contratos, gates, registro de avance y evidencia de esta fase se mantienen en [`docs/fase-3-seguridad-y-usuarios.md`](docs/fase-3-seguridad-y-usuarios.md). Toda operación síncrona de identidad entra por HTTP/REST/OpenAPI al BFF y continúa por gRPC a `identity`; Redis aplica límites de abuso con degradación cerrada; los eventos durables de identidad se persisten en Outbox y se publican a NATS JetStream. Fase 3 está cerrada al 100 %: el propietario confirmó los flujos funcionales y las actualizaciones realtime de esta fase. El propietario confirmó la zona horaria personal, el webhook real de Resend, el correo de cuenta creada, la rotación de la API key, SPF/DKIM/DMARC `PASS`, la validación de contraseña en la interfaz y la respuesta fail-closed HTTP 503 con Redis detenido. La ubicación en Spam se monitorea como operación y no bloquea el cierre de la fase.

- **Fase 3.1 — Autenticación y sesiones:** implementar credenciales, cookies seguras, creación/rotación/revocación de sesiones, protección CSRF, límites contra abuso y cierre de sesión.
- **Fase 3.2 — Alta y recuperación:** implementar registro público, confirmación transaccional de cuenta creada (separada de verificación), verificación de correo por enlace de un solo uso o código manual de ocho dígitos, reenvío limitado, bienvenida posterior a activación, recuperación/restablecimiento de contraseña, tokens de un solo uso, expiración e idempotencia sin enumerar cuentas. El código se persiste como HMAC con secreto del servicio, expira en 15 minutos, admite máximo cinco intentos y se consume una sola vez; limitar adicionalmente por correo y origen con Redis fail-closed. El enlace usa `/verify-email#token=...` en el frontend tenant y se verifica automáticamente sin enviar el token en la petición GET ni retenerlo en historial. Resend envía versiones HTML con estilos en línea, marca XOVYRA y texto plano equivalente, con contenido específico para cada propósito. El remitente transaccional visible es `XOVYRA <no-reply@jnmtortas.com>`; marketing usa una identidad separada. Validar webhooks Svix sobre el cuerpo crudo con deduplicación y procesar entrega, rebote, queja, supresión y baja de marketing. Configurar solo en backend `RESEND_API_KEY`, `RESEND_FROM_EMAIL`, `RESEND_WEBHOOK_SECRET` y, si se habilita marketing, `RESEND_FROM_MARKETING`; mantener secretos fuera de Git y del frontend. Verificar SPF, DKIM y DMARC alineados. Monitorear reputación/quejas en Resend y Google Postmaster Tools como operación continua; la carpeta Spam no es por sí sola un gate de cierre.
- **Fase 3.3 — Cuenta y seguridad personal (CERRADA; aceptación del propietario confirmada):** lectura/actualización del perfil, preferencia personal de zona horaria IANA persistida y aplicada a fechas, sesiones activas, revocación/cierre de otras sesiones, cambio de contraseña, actividad de seguridad y auditoría de acciones sensibles. El propietario confirmó también los flujos realtime de Fase 3. Evidencia en [`docs/fase-3-seguridad-y-usuarios.md`](docs/fase-3-seguridad-y-usuarios.md).
- **Fase 3.4 — Identidad de operador y permisos base:** separar autenticación/autorización de operadores globales de la identidad y sesión de clientes; establecer roles/permisos globales y registrar accesos administrativos. La comprobación de permisos se aplica también a recuperación y WebSocket realtime de plataforma; los permisos por dominio protegerán las rutas operativas cuando se implementen en sus fases correspondientes. La autorización global nunca concede membresía ni acceso implícito a datos de tenant.

**Cierre de fase:** los flujos completos de alta, verificación, inicio, renovación, recuperación y revocación funcionan con persistencia real; las sesiones cliente y operador no se confunden, los casos negativos están probados y los eventos Identity se sincronizan/revocan por el gateway realtime entre sesiones.

### Fase 4 — Multi-tenancy y equipos

**Objetivo:** garantizar aislamiento real entre organizaciones, espacios, usuarios y operadores.

El diseño, los contratos, gates y registro de avance de esta fase se mantienen en [`docs/fase-4-multi-tenancy-y-equipos.md`](docs/fase-4-multi-tenancy-y-equipos.md). El límite tenant canónico inicial es el workspace según §8: no se crea una entidad `organizations` duplicada. Todas las operaciones síncronas pasan por HTTP/REST al BFF y gRPC al módulo dueño; PostgreSQL conserva la autoridad de negocio, Redis se aplica solo a estado efímero/cache con TTL cuando corresponda y los eventos durables se escriben en Outbox y se publican por NATS JetStream. El panel tenant y el panel de administración global permanecen como aplicaciones y sesiones separadas.

- **Fase 4.1 — Workspaces, gestión y contexto activo (CERRADA; aceptación manual confirmada 2026-10-19):** unicidad de nombres por propietario, creación/renombre/selección, eliminación programada restaurable y purga inmediata con revisión de retenciones; solo workspaces activos se pueden seleccionar. Las transiciones usan Outbox/JetStream, journal/WebSocket, TanStack Query y recuperación REST. El propietario confirmó en navegador el recorrido completo de espacios individuales y de equipo.
- **Fase 4.2 — Membresías, invitaciones, colaboración y realtime (CERRADA; aceptación manual confirmada 2026-10-19):** invitaciones y miembros verificados, roles/estados, tareas, comentarios, transferencia de propiedad y protección del último owner. El inbox persistente notifica asignaciones/menciones, invitaciones recibidas/aceptadas, cambio de rol, suspensión/reactivación/revocación, propiedad recibida y eliminación programada/definitiva. La campana y bandeja se actualizan por el coordinador WebSocket/TanStack Query; REST con cursor permite recuperar cambios y la sincronización manual sigue disponible. El propietario confirmó en navegador que las notificaciones llegan sin recarga ni actualización manual. La última entrega pasó formato, `cargo check`, Clippy estricto, lint y build tenant; no se ejecutaron pruebas automatizadas nuevas durante esta revisión documental.
- **Fase 4.3 — Autorización por tenant y preferencias compartidas (CERRADA; pruebas automatizadas y aceptación manual confirmadas 2026-10-20):** aplicar permisos en cada consulta y comando del BFF/módulo, incluidos lectura, edición, eliminación lógica, recuperación y purga. Verificar membresía, rol y acción solicitada en servidor; no confiar en `organization_id` del cliente. Añadir defensa PostgreSQL/RLS donde corresponda sin reemplazar la autorización de dominio. Implementar en `Configuración general` del tenant el idioma/región y la zona horaria predeterminados del workspace, con autorización por rol, validación, auditoría y propagación a recursos compartidos. Los recursos de colaboración actuales presentan fechas con estos valores para todos los miembros; cada futuro módulo de proyectos/contenido los tomará como defaults al crear recursos y respetará overrides autorizados. No duplicarlos en Perfil ni en `/espacios`: Perfil conserva las preferencias personales y `/espacios` la gestión básica y ciclo de vida. Definir precedencia personal/workspace/recurso y persistir los valores compartidos en PostgreSQL; Redis no es fuente de verdad y Outbox/JetStream notifica cambios solo a consumidores que lo necesiten. El propietario confirmó el recorrido en navegador y el guardado de idioma/región y zona horaria; evidencia y procedimiento de regresión constan en `docs/fase-4-multi-tenancy-y-equipos.md`.
- **Fase 4.4 — Aislamiento de recursos asíncronos (CERRADA 2026-10-21):** backend tenant usa HTTP `/api/v1`, BFF→módulo gRPC, Outbox/JetStream y realtime central. La política CORS del bucket `xovyra-ai` quedó configurada y el propietario confirmó en el dominio la carga y eliminación de archivos mediante transferencia directa navegador→R2; la vista de Trabajos mostró los trabajos `asset.delete` como completados, confirmando el ciclo visible de borrado. La verificación automatizada contra R2 también cubrió PUT, HEAD, GET y DELETE y limpió el objeto temporal. El journal admite audiencia `tenant` para eventos `asset_storage.*` y `jobs.*` con workspace, agregado y versión; la prueba real `cargo test -p xovyra-realtime --test journal_real -- --test-threads=1` confirma idempotencia, scope y agregados en PostgreSQL real. Pasaron lint/build tenant, compilación runtime y Clippy estricto runtime/realtime. El BFF registra ruta/status/duración/correlation/trace sin datos sensibles. Los bytes nunca pasan por el backend. Evidencia completa en `docs/fase-4-multi-tenancy-y-equipos.md`.
- **Fase 4.5 — Administración global aislada (CERRADA 2026-10-21):** módulo `platform_control` con schema, migraciones y rol runtime PostgreSQL propios en la misma base física; Identity mantiene sesiones/permisos y Tenancy Access/Asset Storage exponen proyecciones por gRPC. BFF REST `/platform/*` y app independiente `frontend/admin_plataform` incluyen directorio minimizado y paginado, membresías/activos, grants temporales de un activo (máximo una hora), revocación, entrega de URL R2 sin caché ni bytes vía backend, auditoría append-only keyset-paginada, Outbox→JetStream→audiencia `platform` y recuperación TanStack Query/WebSocket/HTTP. Sin membresía tenant ni mutaciones de contenido. La prueba integrada `cargo test -p xovyra-identity --test security_real real_account_security_and_operator_flows_are_isolated_and_publish_events -- --exact --test-threads=1` atraviesa BFF HTTP→gRPC runtime→módulos dueños con PostgreSQL/Redis/JetStream existentes y verifica cookies/audiencias separadas, permisos, directorio, miembros/activos, CSRF, grants, scope, revocación, URL de soporte sin caché, auditoría e invalidación de sesión al revocar rol. `bash scripts/verify-phase-4.5.sh` añade la migración limpia en los 19 schemas, aislamiento de roles, grants/Outbox/journal y Vitest/lint/build admin; comprueba que los errores de readiness y cierres HTTP coincidan en `correlation_id`/`trace_id`. La regresión de journal prueba recuperación de cursor ante pérdida de su fila de control. La prueba Cloudflare R2 real completó PUT, HEAD, preflight OPTIONS, GET y DELETE desde `https://admin-xovyra.jnmtortas.com` y limpió el objeto temporal. El cierre backend/BFF se acepta sin repetición manual del flujo en navegador; se ejecutan Vitest/lint/build admin.

**Cierre de fase:** pruebas negativas y positivas demuestran aislamiento en API, DB, R2/metadatos, eventos, workers y realtime; invitaciones, roles y herramientas de operador respetan sus límites.

### Fase 5 — Monetización y consumo

**Objetivo:** implementar los contratos comerciales, cobros y límites del producto, separados de costes operativos de proveedores.

- **Fase 5.1 — Planes y suscripciones (IMPLEMENTACIÓN EN ACEPTACIÓN):** catálogo/versiones y precios localizados con gates de margen, alta mediante Stripe Checkout, trial configurable, estados por espacio, cancelación/reactivación y cambios al siguiente ciclo sin prorrateo mediante Stripe Subscription Schedules. Integrados BFF HTTP→gRPC, tenant/admin, PostgreSQL, Redis fail-closed, Outbox/JetStream y eventos realtime. Los endpoints administrativos del catálogo están incluidos en OpenAPI. La prueba firmada `cargo test -p xovyra-module-runtime commercial_billing_service` confirma que un webhook actualiza el estado y escribe Outbox de forma idempotente en PostgreSQL real. La prueba `cargo test -p xovyra-module-runtime --test billing_outbox_real` recorre Outbox de billing→JetStream real→journal PostgreSQL, verifica replay, deduplicación e aislamiento por workspace. Aún faltan aceptación Stripe sandbox con credenciales oficiales, ciclo de vida completo con proveedor, invalidación observada por cliente WebSocket autenticado y recuperación tras reinicio Redis/JetStream. Mantener publicación/checkout deshabilitados hasta validar rentabilidad, jurisdicción fiscal y credenciales oficiales de Stripe sandbox. Usar el catálogo de [`docs/decision-comercial-v1.md`](docs/decision-comercial-v1.md) —Básico USD 9/750 créditos, Pro USD 19/1.580, Business USD 49/4.080— solo como hipótesis no publicadas. Matriz técnica y evidencias en [`docs/fase-5-monetizacion-y-consumo.md`](docs/fase-5-monetizacion-y-consumo.md).
- **Fase 5.2 — Pagos y facturas:** integrar Wompi para COP y la cuenta Stripe existente del propietario para USD, validando el estado merchant, país, monedas, tarifas, credenciales y webhooks. Implementar clientes, facturas, pagos, impuestos por jurisdicción, reembolsos, fallos, webhooks firmados, deduplicación, orden fuera de secuencia y conciliación; mostrar subtotal/impuesto/total. No habilitar cobro real sin validar clasificación tributaria y facturación aplicable.
- **Fase 5.3 — Ledger de créditos:** implementar libro mayor inmutable; los créditos son unidades internas de consumo, con valor de referencia versionado inicialmente en USD 0,01, no equivalentes al costo Cloudflare. Paquetes Mini USD 5/400, Plus USD 15/1.250 y Studio USD 50/4.200. Validar plan y paquetes mediante simulaciones antes de publicarlos. Incluir grants tras pago confirmado, compra, reserva, liquidación por consumo, liberación, reembolso/reverso, FIFO por vencimiento, vencimiento de grants mensuales a 60 días y no vencimiento inicial de paquetes comprados sujeto a ley. Cada operación y webhook es idempotente y concurrente.
- **Fase 5.4 — Cuotas, presupuestos y tarifas:** implementar límites efectivos por plan/periodo/usuario/modalidad/proyecto/proveedor, presupuestos y agotamiento/gracia. Versionar tarifas Cloudflare por unidad y costo variable atribuible; mantener objetivo inicial de margen operativo del 30 % antes de impuestos sobre costos medidos y asignados: `P=(C+O+F)/(1-r-m)`, incluyendo comisión PSP una sola vez; distinguirlo del margen neto contable. Mantener snapshots históricos, política fiscal configurable y almacenamiento R2 optativo con saldo monetario independiente.
- **Fase 5.5 — Contratos de consumo y conciliación:** exponer saldo, previsión, uso, historial, factura, límites, estimaciones y cargos conciliados; distinguir créditos del cliente, coste proveedor y precio comercial XOVYRA.

**Cierre de fase:** operaciones comerciales reales o sandbox oficial funcionan con webhooks autenticados, idempotencia, estados coherentes, trazabilidad de tarifas, comisiones, impuestos, saldos separados y prevención de doble cobro/consumo. La cuenta Stripe existente está validada técnicamente, reglas fiscales, cuenta/contrato Wompi, catálogo localizado y simulaciones de rentabilidad están resueltos o mantienen bloqueada la activación de cobros reales.

### Fase 6 — Capacidades multimodales e integraciones

**Objetivo:** habilitar contenido y conexiones generales sin obligar a usar workflows editoriales.

- **Fase 6.1 — Modelo multimodal general:** implementar proyectos, tareas, entradas, activos, resultados y versiones para imagen, video, clip, audio, voz, música, texto y composiciones.
- **Fase 6.2 — Registro de capacidades:** guardar y exponer proveedor/modelo, modalidades de entrada/salida, parámetros tipados, formatos, límites, disponibilidad y versión/fecha de verificación; rechazar capacidades/argumentos no compatibles.
- **Fase 6.3 — Framework de integraciones:** implementar conexión, credenciales cifradas, alcance por tenant, prueba real, reconexión/desconexión, OAuth seguro, webhooks, capacidades, estado y errores normalizados.
- **Fase 6.4 — Operaciones independientes:** exponer generación/edición multimodal independiente y estados de trabajo sin requerir brief, investigación, storyboard ni aprobación editorial; preservar también workflows especializados opcionales.

**Cierre de fase:** proyectos y operaciones independientes usan contratos reales, validan dinámicamente capacidades y respetan permisos/tenant; no hay capacidades asumidas por nombre de modelo.

### Fase 7 — Canales, proyectos y configuración editorial

**Objetivo:** completar la organización del contenido y la preparación de publicación sin acoplarla a generación general.

- **Fase 7.1 — Canales e identidad externa:** implementar CRUD de canales, vinculación OAuth, `state`/PKCE cuando corresponda, credenciales cifradas, metadatos, configuración de idioma/audiencia/estilo, estado, renovación y revocación.
- **Fase 7.2 — Organización editorial:** implementar proyectos, series, campañas, plantillas, briefs, reglas de marca, preferencias de formato, duración y parámetros configurables de Shorts/videos largos.
- **Fase 7.3 — Configuración y permisos de producción:** implementar miembros/roles autorizados para canales/proyectos y configuración de presupuesto, concurrencia, calendario y zona horaria sin imponer metas financieras a proyectos multimodales generales.

**Cierre de fase:** canales y configuración se aíslan por tenant, tokens se protegen y revocan, y proyectos generales pueden coexistir con series/editorial opcionales.

### Fase 8 — Investigación y guiones

**Objetivo:** completar la trazabilidad factual y el proceso de creación/revisión de guiones.

- **Fase 8.1 — Fuentes e investigación:** implementar fuentes, importación permitida, procedencia, fechas, contenido, afirmaciones/cifras y relaciones auditables con tenant/proyecto.
- **Fase 8.2 — Brief y estructura narrativa:** implementar briefs editoriales, investigación vinculada, outline, audiencia, tono, formato y decisiones de generación.
- **Fase 8.3 — Guiones y versiones:** implementar generación real por capacidades habilitadas, borradores, versiones inmutables, trazabilidad de fuentes, revisión factual/editorial, adaptación de formato/duración, títulos, descripciones, CTA y transcripción/subtítulos.
- **Fase 8.4 — Revisión colaborativa:** implementar comentarios, solicitudes de cambios, historial y estados de versión; reservar aprobación formal de publicación a la Fase 12.

**Cierre de fase:** se puede trazar cada afirmación a sus fuentes; las versiones y comentarios conservan autoría, permisos e historial sin pérdida de versiones.

### Fase 9 — Workers AI y catálogo Cloudflare AI/AI Gateway

**Objetivo:** integrar Workers AI alojado por Cloudflare y modelos terceros del catálogo Cloudflare AI/AI Gateway, con adapters, billing, tarifas y capacidades separados y verificables.

- **Fase 9.1 — Adaptador Workers AI:** integrar REST oficial de Workers AI para IDs `@cf/...`, auth server-side, request/response reales, límites, timeouts y errores; conciliar neurons y precios granulares por modelo, incluyendo cuota global de cuenta sin imputarla falsamente al costo unitario de cada tenant.
- **Fase 9.2 — Adapter de terceros / AI Gateway:** integrar los IDs terceros `black-forest-labs/flux-3-image-klein`, `pruna/p-image`, `pruna/p-video` y `minimax/h3-max`; configurar Unified Billing o BYOK server-side y conciliar tarifas del catálogo/factura upstream. Si Unified Billing cobra 5 % al adquirir créditos, contabilizarlo una vez en compra, nunca como neuron o cargo repetido por llamada.
- **Fase 9.3 — Catálogo y capacidades:** registrar los 10 IDs `@cf/...` elegidos y los cuatro IDs de terceros por modalidad, parámetros, límites, status Beta/terms, unidad, precio y fecha/fuente de tarifa; obtenerlos o verificarlos por catálogo y rechazar combinaciones no compatibles.
- **Fase 9.4 — Operación fiable y costes:** integrar idempotencia, errores/rate limits, reintentos seguros, cancelación/async según modelo, observabilidad y registro de consumo/costo en unidades reales para ambos canales.
- **Fase 9.5 — Verificación de proveedor:** probar ambos adapters contra cuenta oficial real/sandbox disponible. Documentar modelos/modalidades no soportados y bloqueos de credenciales sin simular éxito.

**Cierre de fase:** el servicio real solo ofrece capacidades confirmadas, protege credenciales, registra uso y gestiona fallos; las verificaciones externas tienen evidencia de sandbox/cuenta real. Si falta acceso o una credencial, la fase queda abierta y bloquea el avance.

### Fase 10 — Almacenamiento y multimedia

**Objetivo:** implementar el ciclo privado de objetos R2 y el procesamiento audiovisual completo.

- **Fase 10.1 — Dominio de archivos y políticas:** implementar metadatos/propiedad/ACL/versión/estado/ciclo de vida en PostgreSQL; object keys opacas y no controlables por cliente; archivos privados por defecto.
- **Fase 10.2 — Cargas directas R2:** implementar intención, reserva de cuota, URL temporal exacta, staging/cuarentena, multipart, finalización, validación de tamaño/checksum/tipo, promoción a key final y limpieza de cargas huérfanas. Los bytes no pasan por el backend.
- **Fase 10.3 — Descargas, acceso y borrado:** revalidar sesión, tenant, propietario/ACL y permiso en cada operación; emitir autorización GET temporal de objeto exacto, implementar compartir explícito, borrado, retención y auditoría. Probar que un usuario no lea archivos privados de otro.
- **Fase 10.4 — Cuota y cobro R2:** ofrecer almacenamiento persistente optativo con opt-in y saldo monetario separado; medir GB-mes, operaciones y retrieval atribuibles por tenant/propietario/objeto; recarga manual mínima equivalente USD 5, reservar 30 días, conciliar diariamente y alertar al 20 %/siete días; aplicar margen del 30 % sobre precio final incorporando comisión de pago. Tras agotamiento, gracia de 7 días y 30 días de solo lectura/exportación con avisos antes de borrado auditable. Resultados sin persistencia expiran a 24 horas. Mantener separado de créditos IA y cargos de generación.
- **Fase 10.5 — Activos y procesamiento audiovisual:** implementar escenas, imágenes/clips disponibles por capacidades confirmadas, narración, música/efectos, subtítulos, miniaturas, FFmpeg/FFprobe, scratch temporal acotado, composición y renders guardados en R2.
- **Fase 10.6 — Calidad de medios:** validar codecs, streams, duración, audio/video, integridad y calidad; devolver errores accionables y estado del recurso sin publicar archivos no validados.

**Cierre de fase:** flujo real navegador↔R2 está protegido de extremo a extremo; propiedad privada, cuota, markup, procesamiento y resultados funcionan con aislamiento y pruebas de abuso.

### Fase 11 — Orquestación y workers

**Objetivo:** ejecutar operaciones largas y workflows de forma persistente, recuperable y aislada.

- **Fase 11.1 — Modelo persistente de trabajos:** implementar estados, etapas, progreso desconocido/conocido, dependencias, entradas/salidas, tenant, propietario, errores tipados y transiciones válidas en PostgreSQL.
- **Fase 11.2 — Dispatch y ejecución:** implementar workers Rust, leasing/bloqueos, concurrencia configurable, prioridades, límites, cancelación, idempotencia, retry/backoff y dead-letter/operación de errores.
- **Fase 11.3 — Eventos y recuperación:** conectar outbox/inbox con NATS JetStream replicado, consumidores durables idempotentes, contexto tenant/audiencia durable, replay y recuperación tras caída/reinicio sin pérdida ni duplicación; diseñar despliegues con varias réplicas consumidoras y versiones mixtas.
- **Fase 11.4 — Progreso y programación:** integrar trabajos con el gateway WebSocket transversal y el coordinador TanStack Query de ambas apps; exponer progreso/errores/autorización mediante REST snapshot/delta y eventos versionados, cursor/reconexión, orden por agregado y fallback manual. No crear un stream por pantalla.

**Cierre de fase:** los trabajos sobreviven reinicios, no cruzan tenants, toleran duplicados/fallos y reportan estados correctos mediante contratos autorizados.

### Fase 12 — Revisión y aprobación

**Objetivo:** impedir que contenido cambiante o no revisado se publique.

- **Fase 12.1 — Bandeja de control de calidad:** integrar resultados de calidad automática de medios y validaciones editoriales en decisiones de revisión.
- **Fase 12.2 — Versiones y aprobación inmutable:** asociar revisión/aprobación a hash o ID inmutable de versión, usuario, permisos, fecha y resultado; permitir aprobación, rechazo y solicitud de cambios.
- **Fase 12.3 — Enforcement de publicación:** exigir aprobación vigente de la versión exacta en todos los caminos, incluidos workers, reintentos, programación y llamadas internas; invalidarla al cambiar contenido.
- **Fase 12.4 — Colaboración y auditoría:** registrar comentarios, comparaciones, revisiones y cambios con historial y permisos; cubrir concurrencia y acciones duplicadas.

**Cierre de fase:** ninguna ruta puede publicar versiones no aprobadas, alteradas o aprobadas por usuario sin permiso; las pruebas negativas confirman el bloqueo.

### Fase 13 — YouTube y calendario

**Objetivo:** completar el canal especializado opcional de publicación, programación y reconciliación.

- **Fase 13.1 — Integración YouTube:** implementar OAuth y scopes mínimos, renovación/revocación segura, consulta de canales/cuotas y adapter según API oficial.
- **Fase 13.2 — Publicación:** implementar subida/resumable si corresponde, metadatos, miniaturas, programación y publicación idempotente; comprobar aprobación exacta y presupuesto/cuota antes de cada ejecución.
- **Fase 13.3 — Calendario y capacidad editorial:** implementar calendario, zona horaria, series/campañas y configuración de volumen objetivo (15 Shorts y 2 videos largos al día como valores configurables, nunca forzados), duraciones y concurrencia.
- **Fase 13.4 — Estado y conciliación:** reconciliar estado de API y canal, errores, cuotas, reintentos, duplicados y publicaciones realizadas; evitar publicar contenido real durante pruebas.

**Cierre de fase:** publicar/programar en sandbox o entorno autorizado, reconciliar estados y cuotas, y demostrar que los guardrails de aprobación e idempotencia permanecen efectivos.

### Fase 14 — Costes y analítica

**Objetivo:** unificar medición y reportes sin confundir coste operativo con cobro comercial.

- **Fase 14.1 — Ledger de costes/uso:** registrar consumo por operación, proveedor/modelo/unidad, tenant, usuario y proyecto; conservar estimación, respuesta real y conciliación histórica.
- **Fase 14.2 — Presupuestos y alertas:** aplicar presupuestos diarios/mensuales/por proyecto, límites, alertas y suspensión/restauración conforme a suscripción y permisos.
- **Fase 14.3 — Analítica de canal:** implementar métricas y reportes de YouTube, consulta/importación autorizada, rangos/zonas horarias y atribución al canal/proyecto.
- **Fase 14.4 — API de consumo y reportes:** exponer filtros, paginación, agregaciones e informes de créditos, IA, almacenamiento R2 y publicaciones; distinguir estimado, facturado y conciliado.

**Cierre de fase:** conciliaciones no alteran asientos previos, cada importe permite rastrear su origen/tarifa y los reportes respetan tenant y rol.

### Fase 15 — Administración y observabilidad

**Objetivo:** dar operación global segura y visibilidad fiable del sistema.

- **Fase 15.1 — Administración SaaS:** completar contratos `/platform/*` para clientes, organizaciones, membresías/estado, planes, tarifas, suscripciones, pagos, créditos/uso, R2, modelos, límites e incidencias con autorización separada.
- **Fase 15.2 — Soporte y configuración global:** implementar búsqueda/filtros/historial, cambios versionados auditados, acceso temporal de soporte, estado de servicio y gestión de integraciones sin exposición de contenido privado o secretos.
- **Fase 15.3 — Auditoría y notificaciones:** completar eventos de auditoría y preferencias/entrega/errores de notificación para usuario, equipo y operación, con tenant y permisos correctos.
- **Fase 15.4 — Observabilidad y alertas:** instrumentar REST, gRPC, PostgreSQL, Redis, NATS, workers y proveedores con métricas, logs/trazas, dashboards, SLOs configurados y alertas operativas.
- **Fase 15.5 — Salud e incidencias:** implementar health/readiness, diagnóstico seguro, correlación de incidencias, degradación y recuperación sin revelar información sensible.

**Cierre de fase:** consola no cruza autoridad de cliente; operaciones quedan auditadas y el sistema permite detectar y diagnosticar fallos sin filtrar secretos ni datos privados.

### Fase 16 — Pruebas integrales y seguridad

**Objetivo:** demostrar con implementaciones reales que los dominios y sus límites resisten fallos, abuso y concurrencia.

- **Fase 16.1 — Validación funcional completa:** ejecutar pruebas unitarias de reglas puras, integración con servicios reales aislados, migraciones desde cero/upgrade, contratos REST/gRPC/eventos y workflows E2E; sin mocks, fixtures, seeds ni respuestas simuladas.
- **Fase 16.2 — Seguridad y aislamiento:** probar autenticación, autorización, IDOR, tenant, roles de operador, SQL/SSRF/path traversal, credenciales, sesiones, URLs R2, CORS y cargas maliciosas.
- **Fase 16.3 — Integraciones externas reales:** probar proveedores y canales mediante adapters reales en sandbox oficial. No publicar contenido real. Sin sandbox/credencial, dejar ese criterio bloqueado y no simular un aprobado.
- **Fase 16.4 — Resiliencia y rendimiento:** probar concurrencia, idempotencia, límites de recursos, rate limits, timeouts, fallos/reinicio de DB/NATS/R2/proveedores, recuperación de trabajos, backup y restore, carga y caos controlado.
- **Fase 16.5 — Corrección y cierre de defectos:** resolver hallazgos de seguridad, regresión y aceptación; repetir las verificaciones afectadas y actualizar el informe de cobertura y bloqueos.

**Cierre de fase:** todo chequeo ejecutable pasa sin silenciar advertencias ni ignorar pruebas; riesgos y dependencias externas están resueltos o bloquean explícitamente el cierre.

### Fase 17 — Despliegue y documentación

**Objetivo:** entregar una instalación y operación repetibles, seguras y verificadas.

- **Fase 17.1 — Artefactos y configuración:** finalizar Cargo/build, configuración por entorno, plantilla de variables sin secretos, contenedores/scripts y permisos mínimos.
- **Fase 17.2 — Despliegue y actualización:** documentar y verificar instalación desde cero, TLS/proxy/red, servicios API/workers, migraciones coordinadas en la DB compartida, health checks, rollout y recuperación de versión.
- **Fase 17.3 — Backups y recuperación:** automatizar backups de PostgreSQL y R2 según política, retención, acceso y cifrado; ejecutar restauración real y documentar RPO/RTO alcanzados.
- **Fase 17.4 — Documentación de producto y operación:** entregar OpenAPI, contratos gRPC/eventos, arquitectura, variables, matrices de permisos/planes/capacidades, manual de administración, runbooks, recuperación ante desastres y dependencias externas.
- **Fase 17.5 — Revisión final de aceptación:** recorrer todos los criterios de la sección 34 y entregables de la sección 35 con evidencia de compilación, pruebas, instalación y servicios reales; publicar el informe final de terminado/bloqueado.

**Cierre de fase:** una instalación limpia puede desplegarse, migrarse, operar, respaldarse y restaurarse siguiendo la documentación; todos los criterios globales tienen evidencia. Si un bloqueo externo deja algún criterio sin evidencia, la Fase 17 y el roadmap permanecen abiertos y se documenta qué impide completarlos.

No omitas fases ni subfases. No reduzcas entregables. Una subfase bloqueada mantiene abierta su fase y no autoriza a pasar a la siguiente.

## 34. Criterios de aceptación finales

El backend solo podrá declararse completo cuando:

1. Compile sin errores y sin advertencias relevantes no justificadas.
2. Todos los módulos se conecten a una única base PostgreSQL; sus migraciones independientes se apliquen coordinadamente desde una base vacía y durante upgrades; y se respete la propiedad de schemas/tablas sin acceso cruzado ni JOINs SQL entre módulos.
3. El registro, verificación de correo y onboarding creen usuario, primer espacio y membresía de forma segura e idempotente.
4. Las pruebas demuestren aislamiento entre tenants en API, base de datos, archivos, eventos y workers, incluidos intentos IDOR.
5. La consola de plataforma tenga autorización independiente y no otorgue membresías de cliente implícitas.
6. Membresías, invitaciones y permisos de espacio funcionen con roles configurables y auditoría.
7. Planes, suscripciones, pagos, facturas, créditos y cuotas tengan contratos documentados, estados coherentes e idempotencia probada.
8. Los webhooks comerciales verifiquen firma, toleren duplicados y eventos fuera de orden, y concilien el estado.
9. Un ledger de crédito registre y concilie operaciones sin duplicar consumo bajo concurrencia, reintentos o cancelación.
10. El modelo admita proyectos multimodales generales y workflows editoriales opcionales.
11. Los clientes puedan consultar capacidades verificadas/versionadas por proveedor y los parámetros incompatibles se rechacen antes de invocar servicios externos.
12. Las integraciones y credenciales tengan alcance correcto, aislamiento y pruebas de autorización.
13. Los objetos permanentes estén almacenados en R2 privado; disco local se limite a scratch temporal.
14. Las cargas/descargas transfieran bytes directamente navegador↔R2 y el backend autentique, autorice, limite, firme y valide el ciclo de vida sin proxyar bytes.
15. Las autorizaciones de objeto sean temporales y específicas, y nunca expongan credenciales permanentes ni den acceso a objetos de otro usuario/tenant; pruebas negativas cubran claves adivinadas y URLs no autorizadas.
16. Una carga PUT que siga vigente no pueda sobrescribir un objeto final validado o publicado.
17. CORS sea restrictivo y buckets privados no tengan acceso anónimo ni listados públicos.
18. El uso de almacenamiento se mida y concilie por tenant y propietario conforme a una tarifa comercial versionada; cargas incompletas, borrados, versiones y fallos respeten cuota y facturación documentadas.
19. Los diez modelos Workers AI `@cf/...` seleccionados y los cuatro modelos terceros estén integrados mediante sus canales correspondientes, con costos, capacidades, términos, límites y unidades reales verificados. Workers AI se concilia en neurons y tarifas por modelo; los modelos terceros se concilian por Unified Billing o BYOK. Ninguna tarifa se mezcla entre canales.
20. El margen operativo objetivo sea 30 % del precio final antes de impuestos sobre costos medidos/asignados (P=(C+O+F)/(1-r-m)), sin confundirlo con margen neto; comisiones se registran una sola vez. Cada asiento conserve modelo/objeto, unidades, moneda, tarifas/costos vigentes, comisión, conversión/margen y precio aplicado sin recalcular cargos históricos. Generación use créditos con conversión versionada y almacenamiento persistente R2 sea opt-in con saldo monetario independiente; costos Cloudflare reales y cargos de cliente se concilien por separado.
21. Las operaciones directas multimodales funcionen sin proyecto editorial, y la composición/workflow de video sean opcionales.
22. La administración global permita gestionar organizaciones, planes, modelos, tarifas, límites e incidencias con permisos y auditoría separados del acceso cliente.
23. El sistema pueda completar flujos de prueba contra proveedores reales usando sus entornos oficiales de prueba/sandbox; ningún proveedor simulado, mock o respuesta ficticia cuenta como integración o aceptación.
24. Las integraciones reales sigan contratos oficiales y se indiquen las que requieren credenciales.
25. FFmpeg genere videos válidos durante pruebas del workflow audiovisual opcional usando el procesamiento real y recursos creados transitoriamente, sin archivos prefabricados de muestra.
26. Los trabajos sobrevivan a reinicios y recuperen estados pendientes manteniendo el tenant correcto.
27. El coste y consumo distingan estimaciones de datos confirmados.
28. Las aprobaciones estén vinculadas a versiones concretas.
29. Ningún camino de ejecución permita publicar sin aprobación vigente.
30. La publicación se integre con YouTube cuando existan credenciales y permisos válidos.
31. Los backups y la restauración estén documentados y probados.
32. Existan pruebas automatizadas, documentación OpenAPI y manual de operación.
33. No queden rutas críticas con implementaciones falsas, TODO o respuestas de demostración.

## 35. Entregables

Al finalizar, entrega:

- Código fuente completo.
- Arquitectura documentada.
- Esquema y migraciones PostgreSQL.
- Modelo de amenazas y garantías de aislamiento multi-tenant.
- Contratos de identidad, espacios, membresías, onboarding y consola de plataforma.
- Contratos de planes, suscripciones, facturación, ledger de créditos y cuotas.
- Catálogo versionado de capacidades multimodales y documentación de proveedores compatibles.
- Procedimientos de conciliación de pagos/consumo y atención de eventos duplicados.
- Tarifario Cloudflare versionado por modelo/unidad y tarifa R2, catálogo de pagos/impuestos versionado, fórmula de margen operativo inicial del 30 % con costos asignados, comisiones incluidas una sola vez, conversión de créditos, saldo R2 separado y política de gracia/retención.
- Consola/API de administración SaaS global con matriz de permisos y auditoría.
- Contratos y documentación de operaciones multimodales independientes, composición y workflows opcionales.
- API documentada.
- Configuración de desarrollo y producción.
- Integraciones externas.
- Workers.
- Servicio de almacenamiento Cloudflare R2 con transferencias directas seguras.
- Esquema de cuotas, uso medido y cargos de almacenamiento por tenant/propietario.
- Adaptadores independientes para Workers AI y modelos terceros del catálogo Cloudflare AI/AI Gateway; catálogo inicial de diez modelos `@cf/...` y cuatro terceros, con capacidades, estados, términos y tarifas verificadas/versionadas.
- Adaptador de correo Resend para notificaciones transaccionales y campañas con consentimiento, webhooks autenticados, idempotencia y estados de entrega observables.
- Sistema de producción audiovisual.
- Sistema de revisión y aprobación.
- Publicación en YouTube.
- Pruebas automatizadas.
- Documentación de instalación.
- Manual de administración.
- Manual de recuperación ante desastres.
- Ejemplo de configuración sin secretos.
- Matriz de variables de entorno.
- Matriz de roles y permisos.
- Lista de proveedores soportados y capacidades verificadas.
- Matriz de planes, límites y políticas comerciales configuradas (sin secretos ni valores inventados).
- Informe de funcionalidades terminadas y pendientes por dependencias externas.
- Resultados reales de compilación y pruebas.

Comienza con la auditoría del repositorio. Después implementa todas las fases hasta entregar el sistema completo. No sustituyas la implementación por una explicación de cómo debería hacerse.
