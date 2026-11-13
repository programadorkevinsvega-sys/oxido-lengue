# Auditoría del estado del proyecto — 2026-10-27

## Resultado ejecutivo

El roadmap y los documentos de fase declaran cerradas Fases 1–4.5, con Fase 1 cerrada como auditoría/diseño y las fases 2–4.5 como implementación. El siguiente trabajo de producto definido es Fase 5.1 — Planes y suscripciones. La revalidación inicial encontró una barrera de configuración en PostgreSQL y errores estructurales de TypeScript ya corregidos en el frontend admin. Tras corregir el aprovisionamiento y completar la migración de la base de desarrollo, la aceptación automatizada de Fase 4.5 pasó de punta a punta el 2026-10-27.

La primera revalidación se detuvo porque no estaba corriendo el PostgreSQL dedicado de XOVYRA y el puerto 5432 pertenecía a `payvia-shared-infra-postgres-1`; no se modificó esa instancia. El usuario inició después el Compose de `backend`, que creó `backend-postgres-1`, Redis y NATS. El primer arranque de PostgreSQL falló porque Compose no pasaba las variables requeridas por el init script. Corregido eso, la aceptación descubrió dos desajustes adicionales: los scripts usaban el nombre fijo `postgres` en vez de resolver el contenedor de servicio activo, y el bootstrap no aprovisionaba los roles runtime de `notifications` y `realtime`. También faltaban las migraciones de schemas/tablas en la base recién creada y el test de identidad asumía que el stream `IDENTITY_EVENTS` ya existía. Se corrigieron esos puntos sin borrar volúmenes, se alinearon roles con el `.env` de desarrollo, se ejecutó el migrador y el test inició el runtime antes de consultar el stream que este provisiona.

## Verificaciones ejecutadas

| Área | Resultado observado |
|---|---|
| Backend formato | `cargo fmt --all --check`: pasó. |
| Backend calidad/compilación | Clippy estricto para realtime, platform-control, runtime, identity y BFF; build de BFF/runtime: pasó. |
| Unit tests BFF | 9 pasaron. |
| PostgreSQL/Realtime | Las 3 pruebas `journal_real` pasaron contra PostgreSQL del Compose XOVYRA. |
| Base vacía/migraciones | `scripts/verify-empty-database.sh` pasó: creó una base temporal en el servicio Compose activo, aplicó las migraciones dos veces, verificó grants/aislamiento y eliminó únicamente la base temporal. |
| Aceptación HTTP→BFF→gRPC | Pasó contra PostgreSQL/Redis/JetStream reales; 7 cierres HTTP fallidos quedaron emparejados con errores operativos por `correlation_id` y `trace_id`. |
| Fase 4.5 integral | `bash scripts/verify-phase-4.5.sh` pasó: 9 tests BFF, 3 integraciones journal, 2 pruebas de grants y 1 de proyección realtime, aceptación de identidad/operador y Vitest/lint/build admin. Lint: 0 errores y 6 advertencias Fast Refresh compartidas. |
| Frontend tenant | Instalación congelada por lockfile; 2 tests, TypeScript `tsc --noEmit`, lint y build: pasaron. |
| Frontend admin | Instalación congelada por lockfile; 1 test, TypeScript `tsc --noEmit`, lint y build: pasaron después de corregir 7 accesos a propiedades de tipo índice en dos componentes compartidos. |
| Lint admin | 0 errores y 6 advertencias Fast Refresh en componentes UI compartidos. El build también informa advertencias de plugin Vite y tamaño de chunks; no impidieron la compilación. |
| Integraciones externas | R2 y Resend reales no forman parte de esta ejecución de `verify-phase-4.5.sh`; las pruebas reales documentadas anteriormente siguen siendo la evidencia de esos proveedores. |

## Hallazgos de documentación corregidos

- `roadmap.md` decía que la Fase 4 global seguía abierta únicamente por 4.5, aunque el documento de fase registraba Fases 4.1–4.5 y la Fase 4 global como cerradas.
- `PROJECT_RULES.md` y dos descripciones de estado en el documento de Fase 2 todavía decían que `realtime` estaba pendiente. El runtime actual declara 19 contextos e incluye `realtime` y `platform_control`; las menciones históricas se aclararon.
- `ARCHITECTURE_RULES.md` debe seguir siendo idéntico a `reglas-estrictas-de-arquitectura-monolito-modular-rust-universal.md`.

## Hallazgos pendientes

1. Mantener `docker compose up -d` como paso explícito del desarrollador y usar `scripts/bootstrap-existing-postgres.sh` si el volumen ya estaba inicializado antes de corregir el init script. El bootstrap localiza PostgreSQL por servicio Compose y alinea roles sin borrar datos.
2. La base física recién creada necesita `cargo run -p xovyra-migrator` antes de ejecutar los servicios o las pruebas de integración.
3. Un análisis estático previo con `rg` encontró 64 sitios Rust que registran `error_type` basado en el tipo Rust de la causa. La búsqueda por sí sola no demuestra que los 64 eventos sean incorrectos, pero la revisión global de esos sitios sigue pendiente: clasificar cada uno, asegurar código/categoría, operación, dependencia, resultado, duración e IDs de correlación cuando estén disponibles y añadir pruebas de logs para las rutas afectadas.
4. R2 y Resend reales son pruebas separadas de la aceptación automatizada backend/BFF y requieren bucket/buzón de prueba; la evidencia previa está documentada aparte.

## Estado de avance recomendado

La aceptación automatizada backend/BFF/admin de Fase 4.5 quedó revalidada el 2026-10-27 y no requiere reabrirse. La inspección transversal de los 64 eventos `error_type` queda como deuda de observabilidad global, independiente del cierre de Fase 4.5. El siguiente alcance funcional del roadmap sigue siendo Fase 5.1; documentar esa deuda transversal no invalida los gates de fase que ya pasaron.
