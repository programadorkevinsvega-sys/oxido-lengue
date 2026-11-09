# Fase 1 — Auditoría y diseño del backend XOVYRA

## Estado de la fase

**Estado: cerrada al 100 % como fase de auditoría y diseño.** Las cuatro subfases 1.1–1.4 tienen entregables documentales completos. Esta declaración no significa que exista backend implementado. La Fase 2 inicia su ejecución documentada en [`fase-2-infraestructura-y-base-de-datos.md`](fase-2-infraestructura-y-base-de-datos.md); no se inicia Fase 3 hasta que cierre Fase 2 al 100 %.

Documentos de referencia: [`AGENTS.md`](../AGENTS.md), [`roadmap.md`](../roadmap.md), [`reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md`](../reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md), [`ARCHITECTURE_RULES.md`](../ARCHITECTURE_RULES.md), [`PROJECT_RULES.md`](../PROJECT_RULES.md).

## Fase 1.1 — Inventario y diagnóstico

### Estado observado del repositorio

- El repositorio es un monorepo en la rama `main`. Al comenzar la auditoría estaba limpio y sincronizado con `origin/main`.
- `backend/` contiene únicamente `AGENTS.md`, `roadmap.md` y el documento de reglas estrictas. No contiene `Cargo.toml`, `Cargo.lock`, crates, fuentes Rust, migraciones, pruebas, configuración, workflows CI, Dockerfiles, scripts de despliegue ni documentación de API/operación implementada.
- Por tanto, no hay backend compilable que preservar ni se pueden ejecutar `cargo check`, Clippy o suites de backend en esta fase. No se generó código para aparentar una implementación.
- El frontend sí tiene una aplicación React/TypeScript con TanStack Start/Router, `src/lib/api.ts`, módulos tenant bajo `src/routes/_shell/` y rutas de plataforma bajo `src/routes/plataforma/`. El `package.json` describe un único paquete Vite; la separación física en dos builds/apps que exige el roadmap frontend no está materializada todavía. Es una dependencia de coordinación frontend, no una tarea que se resuelva dentro de este backend.
- El cliente actual `src/lib/api.ts` es centralizado y usa cookies, correlation ID y timeout, pero interpreta JSON como el genérico `T` mediante casts y no se encontró un OpenAPI de backend que lo valide/genere. El backend deberá publicar contratos OpenAPI estables; el endurecimiento del cliente pertenece al roadmap frontend.
- La API real, servicios externos conectados, base de datos, esquema, credenciales, despliegues y estados operativos no están presentes. Ninguno se reporta como disponible o validado.

### Inventario de capacidades actuales

| Capacidad backend | Estado en el repositorio | Tratamiento en el diseño |
|---|---|---|
| Workspace Cargo / runtime Axum / BFF | Ausente | Diseñado en fases 2.1–2.4 |
| Módulos Rust por bounded context | Ausentes | Propiedad propuesta en `PROJECT_RULES.md` |
| PostgreSQL, schemas y migraciones | Ausentes | Una base física, schema/migraciones por módulo; fase 2.2 |
| gRPC / NATS / Outbox | Ausentes | Contratos internos requeridos desde el diseño; fase 2.3 |
| Identidad, sesiones y permisos | Ausentes | Fase 3 |
| Tenants, membresías y aislamiento | Ausentes | Fase 4 |
| Suscripciones, pagos, créditos y ledger | Ausentes | Fase 5 |
| Modelo multimodal y catálogo de capacidades | Ausentes | Fases 6 y 9 |
| Integraciones externas | Ausentes | Fases 6, 9 y 13; requieren contratos/cuentas reales |
| R2 privado y transferencias directas | Ausentes | Fase 10; no se pasan bytes por el backend |
| Jobs/workers y workflows | Ausentes | Fase 11 |
| Control de calidad, aprobación y publicación | Ausentes | Fases 10, 12 y 13 |
| Administración SaaS, auditoría/observabilidad | Ausentes | Fase 15 |
| Pruebas, CI, despliegue, backups/runbooks | Ausentes | Fases 16 y 17 |

## Fase 1.2 — Mapa de requisitos y dominios

La subdivisión conserva el alcance funcional del roadmap. Esta matriz traza las secciones funcionales completas a las fases/subfases que las implementarán; los criterios numerados se trazan después en otra tabla.

| Requisito del roadmap | Subfases propietarias principales |
|---|---|
| §3 Volumen editorial opcional, series/campañas, canales, zona horaria, presupuestos y concurrencia | 7.2–7.3, 13.3, 14.2 |
| §4 Stack, límites de módulos, BFF, PostgreSQL compartido y contratos internos | 1.3, 2.1–2.4 |
| §5 R2 privado, autorizaciones, carga/descarga directa, ACL, staging, cuota y retención | 10.1–10.4, 17.3 |
| §6 Estructura Cargo y separación por capas | 1.3, 2.1–2.3 |
| §7 Identidad, sesiones, RBAC y seguridad de usuario/operador | 3.1–3.4, 4.3, 4.5 |
| §8 Tenant canónico, membresías, autorización y aislamiento end-to-end | 4.1–4.5 |
| §9 Registro, verificación, onboarding, catálogo comercial y estado público | 3.2, 4.1, 5.1, 15.5 |
| §10 Planes, suscripciones, pagos, facturas, créditos, pricing, cuotas y conciliación | 5.1–5.5, 10.4, 14.1–14.4 |
| §11 Proyectos multimodales y capacidades verificadas | 6.1–6.4, 9.2 |
| §12 Equipos, integraciones, secretos, OAuth y webhooks | 4.2–4.4, 6.3, 7.1 |
| §13 Usuarios, preferencias, canales, credenciales y configuración editorial | 3.3, 7.1–7.3, 13.1 |
| §14 Proyectos/series/CRUD, búsquedas y configuración multimodal/editorial | 6.1, 7.2–7.3 |
| §15 Investigación financiera, fuentes, afirmaciones, cifras y trazabilidad | 8.1–8.3 |
| §16 Guiones, versiones, revisión factual/editorial y comentarios | 8.2–8.4, 12.2, 12.4 |
| §17 Adaptadores IA externos, Cloudflare inicial, capacidades, coste y resiliencia | 6.2–6.3, 9.1–9.4 |
| §18 Generación independiente, composición, audiovisual, TTS/audio/subtítulos y FFmpeg | 6.1, 6.4, 10.5 |
| §19 Control de calidad con estados verificables | 10.6, 12.1, 16.1–16.2 |
| §20 Jobs persistentes, retries, leases, límites, cancelación y recuperación | 11.1–11.4, 16.4 |
| §21 Revisión humana, aprobación inmutable e enforcement | 12.1–12.4, 13.2 |
| §22 YouTube OAuth, subida, metadatos, progreso, cuotas y reconciliación | 13.1–13.4, 16.3 |
| §23 Calendario, recurrencia, zonas IANA y prevención de duplicados | 11.4, 13.3–13.4 |
| §24 Costes, ledgers, presupuestos, proyección y límites | 5.3–5.5, 10.4, 14.1–14.4 |
| §25 Analítica YouTube y procedencia de métricas | 14.3–14.4 |
| §26 Notificaciones internas y externas | 15.3 |
| §27 Auditoría, logs, métricas, traces y alertas | 3.3–3.4, 4.5, 15.3–15.5 |
| §28 REST/OpenAPI cliente y `/platform/*`, errores, realtime e idempotencia | 2.3–2.4 y contratos de todos los dominios; pruebas 16.1 |
| §29 Modelo relacional, ownership, integridad y migraciones | 2.2 y migraciones propietarias de todas las subfases de dominio |
| §30 Seguridad adicional, SSRF, SQL, secretos, R2, backup y retención | 2.4, 10.1–10.4, 16.2, 17.3 |
| §31 Configuración, instalación, TLS, scripts, backup/restore y recuperación | 2.1–2.4, 17.1–17.4 |
| §32 Pruebas de unidad, integración real, contratos, E2E, rendimiento, chaos y seguridad | 16.1–16.5; validación incremental de cada subfase |
| §33 Fases y subfases, cierre y trazabilidad | 1.2–1.4 y cada gate de §33 del roadmap |
| §34 Criterios de aceptación globales 1–33 | Matriz siguiente |
| §35 Entregables finales | 17.4–17.5, sustentados por dominios y fases anteriores |

### Trazabilidad de criterios de aceptación (§34)

| Criterio | Subfases que lo verifican |
|---:|---|
| 1. Compilación sin errores/advertencias relevantes | 2.1, 16.5, 17.5 |
| 2. PostgreSQL único y migraciones separadas/coordinadas | 2.2, 16.1, 17.2 |
| 3. Registro, verificación y primer workspace idempotentes | 3.2, 4.1, 16.1 |
| 4. Aislamiento tenant en API/DB/archivos/eventos/workers | 4.3–4.4, 10.3, 11.3, 16.2 |
| 5. Autorización plataforma independiente | 3.4, 4.5, 15.1, 16.2 |
| 6. Membresías, invitaciones y roles configurables/auditados | 4.2–4.3, 15.3, 16.2 |
| 7. Suscripciones, pagos, facturas, créditos y cuotas | 5.1–5.5, 16.1 |
| 8. Webhooks comerciales seguros/idempotentes | 5.2, 16.1–16.2 |
| 9. Ledger de crédito sin doble contabilización | 5.3, 14.1, 16.4 |
| 10. Proyectos multimodales y workflow opcional | 6.1, 6.4 |
| 11. Capacidades/modelos verificados y parámetros válidos | 6.2, 9.2 |
| 12. Credenciales e integraciones aisladas | 4.3, 6.3, 16.2 |
| 13. Archivos privados permanentes en R2 | 10.1–10.3 |
| 14. Transferencia navegador↔R2 con control backend | 10.2–10.3, 16.2 |
| 15. Autorización temporal y aislamiento de objeto | 10.3, 16.2 |
| 16. URL PUT no sobrescribe el objeto final | 10.2, 16.2 |
| 17. CORS estricto y sin objetos públicos | 10.2, 16.2 |
| 18. Medición/cobro R2 por tenant/propietario | 5.4, 10.4, 14.1, 14.4 |
| 19. Cloudflare Workers AI real y capacidades verificadas | 9.1–9.4, 16.3 |
| 20. Markup 30 % versionado para IA y R2 | 5.4, 10.4, 14.1 |
| 21. Generación directa sin flujo editorial obligatorio | 6.1, 6.4 |
| 22. Administración global SaaS sin privilegio tenant implícito | 4.5, 15.1–15.2 |
| 23. Pruebas proveedor real en sandbox, nunca simulado | 9.4, 16.3 |
| 24. Integraciones bajo contratos oficiales y credenciales documentadas | 6.3, 9.4, 13.1, 16.3 |
| 25. Render FFmpeg real y recurso generado durante prueba | 10.5, 16.1 |
| 26. Jobs recuperables con tenant correcto | 11.1–11.3, 16.4 |
| 27. Estimación separada de consumo/coste confirmado | 5.5, 9.3, 14.1–14.4 |
| 28. Aprobación ligada a versión concreta | 12.2, 12.4 |
| 29. Ningún camino publica sin aprobación vigente | 12.3, 13.2, 16.1–16.2 |
| 30. YouTube real con credenciales/permisos | 13.1–13.4, 16.3 |
| 31. Backup y restore documentados/probados | 17.3, 16.4 |
| 32. Pruebas, OpenAPI y manual | 16.1–16.5, 17.4–17.5 |
| 33. Sin rutas falsas/TODO/demo críticos | Todas las subfases; gate 16.5 y 17.5 |

## Criterio realtime del mapa de fases

Cada dominio del mapa de requisitos debe especificar cambios persistidos visibles, evento Outbox versionado, consumidor JetStream, scope/audiencia autorizada, invalidaciones TanStack Query y recuperación REST por cursor/snapshot. Esta matriz se repite por fase/subfase y no se asigna todo el trabajo realtime a una sola fase futura. Los dominios implementados antes del gateway se integran al mismo contrato al habilitarlo, sin crear transportes paralelos.

## Fase 1.3 — Diseño arquitectónico y de seguridad

### Topología

```text
Frontend tenant ─┐                         ┌─ gRPC ─ bounded-context crates
                 ├─ REST/OpenAPI ─ BFF ────┤
Frontend platform┘                         ├─ gRPC ─ otros módulos
                                           └─ REST + WebSocket transversal según contrato

Módulos ── PostgreSQL físico compartido (schema/migraciones por módulo)
Módulos ── Outbox ── NATS JetStream ── módulos consumidores

Browser ── bytes directamente ── Cloudflare R2 privado
   └──── API control/autorizar ── BFF ── gRPC ─ storage module
```

La instalación inicial permanece como monolito modular; las fronteras gRPC y NATS siguen existiendo aunque los procesos se ejecuten en un mismo host. R2 es la excepción de transporte: el backend autoriza y valida, pero no retransmite los bytes.

### Límites de datos

1. Workspace es el tenant canónico inicial. Toda entidad de cliente declara su tenant/owner; no se concede autoridad por un ID aportado por navegador.
2. Cada uno de los bounded contexts de `PROJECT_RULES.md` tiene un schema lógico propio dentro de una única base PostgreSQL física y es propietario exclusivo de sus tablas/migraciones.
3. No hay acceso cruzado de tablas, JOINs ni transacciones SQL entre módulos. IDs externos se validan por contrato gRPC; consistencia asíncrona usa eventos versionados y proyecciones propias.
4. Migraciones de módulo viven con el módulo y se aplican coordinadamente; roles/permisos de sistema son configuración de producción instalada por migración de su dueño, no datos de demostración.
5. Credenciales y secretos no se guardan en repositorio, navegador, logs ni filas en claro; persistir solo referencia cifrada/gestionada según el sistema de secretos elegido.

### Threat model inicial

| Amenaza | Límite/mitigación exigida | Evidencia planificada |
|---|---|---|
| IDOR o `workspace_id` falsificado | Resolver membresía y permiso en cada comando/consulta del módulo | 4.3–4.4, 16.2 |
| Lectura de archivo ajeno/adivinar object key | Keys opacas, metadata/ACL en storage module, URL temporal exacta | 10.1–10.3, 16.2 |
| Abuso de URL firmada vencida/reutilizada | TTL corto; reconocer que bearer URL puede reutilizarse hasta expirar; staging y key final distinta | 10.2–10.3, 16.2 |
| Tenant perdido en worker/retry/evento | Persistir tenant en job/event envelope y revalidar antes de ejecutar | 4.4, 11.1–11.3, 16.2 |
| Operador obtiene acceso implícito tenant | Auth plane separada, soporte temporal autorizado y auditado | 3.4, 4.5, 15.1–15.3 |
| Doble cobro/consumo por reintento/webhook | Idempotency keys, ledger append-only, deduplicación y conciliación | 5.2–5.5, 14.1, 16.4 |
| Evento perdido entre commit y NATS | Outbox en transacción del módulo, retry/ack manual, consumer idempotente | 2.3, 11.3, 16.4 |
| Credencial filtrada por logs/API | Secret manager, privilegio mínimo, redacción y respuesta segura | 3.1–3.4, 6.3, 9.1, 16.2 |
| SSRF/archivos/comandos maliciosos | Validación destino y contenido, límites y procesos con args separados | 10.2, 10.5, 16.2 |
| Publicación no aprobada o versión alterada | Snapshot/hash aprobados y comprobación justo antes de publicar | 12.2–12.3, 13.2, 16.2 |

### Contratos iniciales y decisiones arquitectónicas

- **ADR-001 — Monolito modular con límites remotos:** Rust Cargo workspace; módulos crate independientes; BFF Axum solo REST/adaptación; gRPC síncrono y NATS JetStream/Outbox asíncrono desde el inicio; despliegue inicial junto en infraestructura controlada.
- **ADR-002 — PostgreSQL físico único:** una sola base; ownership por schema/tablas y migraciones de módulo; coordinación de aplicación en despliegue; nada de acceso cruzado, JOINs, FK o transacción cruzada.
- **ADR-003 — Superficie HTTP doblemente aislada:** APIs cliente `/api/v1` y operador `/platform/*`; dos aplicaciones frontend separadas, autentificación/sesiones/autorización distintas; OpenAPI es contrato REST.
- **ADR-004 — Control de objetos en backend, bytes directos:** storage module autoriza y emite URLs acotadas tras verificar tenant/owner/ACL/cuota; navegador transfiere bytes directamente a R2 privado.
- **ADR-005 — Proveedor y precio verificables:** Workers AI/R2 son integraciones iniciales; capacidades y tarifas oficiales versionadas; markup 1,30; ningún precio/capacidad actual se inventa ni se hardcodea sin vigencia.
- **ADR-006 — Evidencia real de pruebas:** sin mocks, fakes, fixtures, seeds de prueba ni respuestas simuladas; servicios reales en entorno aislado y sandboxes oficiales. Sin acceso, el criterio queda bloqueado.
- **ADR-007 — Tenant canónico:** workspace es límite de autorización y facturación inicial. Una jerarquía empresarial superior requiere decisión explícita, no una tabla `organizations` redundante con semántica ambigua.

## Fase 1.4 — Plan de ejecución y contratos

### Orden de implementación

La ejecución sigue las 17 fases y 74 subfases del §33 del roadmap. Dependencias principales:

```text
F1 auditoría/diseño
  → F2 runtime/contratos/DB
  → F3 identidad
  → F4 tenant y permisos
  → F5 SaaS comercial
  → F6 capacidades/integraciones
  → F7 proyectos/canales
  → F8 research/editorial
  → F9 Workers AI
  → F10 R2/media
  → F11 jobs
  → F12 aprobación
  → F13 YouTube/calendario
  → F14 costes/analytics
  → F15 administración/observabilidad
  → F16 pruebas integrales/seguridad
  → F17 despliegue/documentación
```

La fase de productos puede paralelizar únicamente dominios independientes dentro de esa misma fase, nunca saltándose la regla de cierre del 100 %. F9 necesita la salida de F6; F10 necesita F2/F4/F5; F11 necesita F2/F4/F6; F12 necesita contenido y media; F13 necesita integración/canales, jobs y approval; F14 integra ledgers previos; F16/17 cierran el producto completo.

### Hitos verificables de la Fase 1

| Subfase | Entregable de esta auditoría/diseño | Estado/evidencia |
|---|---|---|
| 1.1 | Inventario, estado inicial y diagnóstico | Cerrada: §1.1 de este documento y `git ls-files` del backend (solo documentos, sin fuentes ni configuración) |
| 1.2 | Mapa completo de secciones funcionales y criterios de aceptación | Cerrada: matrices de trazabilidad de §1.2; criterios 1–33 asignados |
| 1.3 | Diseño de topología, contexts, ownership, contratos y amenazas | Cerrada: `PROJECT_RULES.md` y §1.3 de este documento |
| 1.4 | Dependencias, decisiones, secuencia y gates de ejecución | Cerrada: decisiones ADR, lista de insumos pendientes y §1.4 de este documento |

### Bloqueos conocidos para fases posteriores

1. No existe código backend inicial; por eso la primera fase de programación será Fase 2, no una ampliación improvisada de Fase 1.
2. Proveedor de pagos, correo transaccional, proyecto OAuth de YouTube, cuentas/configuración de Cloudflare y hosting/observabilidad de producción no están especificados ni disponibles en el repositorio. Resolver cada entrada antes de cerrar su subfase correspondiente, según `PROJECT_RULES.md`.
3. El frontend en el repositorio aún no representa dos builds independientes como lo exige su propio roadmap; coordinar ese trabajo con el agente/frontend antes de consumir sus contratos de producción.
4. Ningún modelo de Workers AI ni tarifa concreta se certifica en esta fase. Debe verificarse contra cuenta y documentación oficial durante Fase 9.

Estos bloqueos no invalidan los entregables de diseño, pero no se pueden presentar como integraciones completas ni permiten cerrar fases posteriores.

## Cierre de la Fase 1

- [x] 1.1 Inspección del repositorio y diagnóstico del estado real.
- [x] 1.2 Trazabilidad de las secciones funcionales y 33 criterios de aceptación sin retirar funcionalidades.
- [x] 1.3 Diseño coherente con los límites modulares, PostgreSQL compartido, gRPC, NATS/Outbox, BFF y R2.
- [x] 1.4 Plan, decisiones, dependencias externas, contratos iniciales y gates por fase documentados.
- [x] Reglas universales accesibles bajo `ARCHITECTURE_RULES.md` y decisiones XOVYRA bajo `PROJECT_RULES.md`.
- [x] No se modificó ni se fingió código del backend en la fase de diseño.
- [x] Formato y enlaces locales de los documentos comprobados; no hay pruebas de backend que ejecutar porque no existe implementación/backend Cargo.

**Resultado:** Fase 1 cerrada. La siguiente fase autorizada por secuencia es Fase 2; su plan y gates están documentados en el informe específico de Fase 2.
