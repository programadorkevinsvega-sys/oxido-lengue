# Política de dependencias y licencias

**Estado:** vigente para el workspace de Óxido y sus herramientas internas. Este documento fija las reglas que aplican `cargo-deny` y `cargo-audit`; no declara que el árbol de dependencias esté libre de riesgos.

**Alcance:** dependencias directas y transitivas declaradas en `Cargo.lock`, dependencias de `tools/validation-report`, `fuzz/` y los fixtures de prueba. Las acciones de CI y los artefactos distribuidos se tratan en el [informe de Fase 12](fase-12-seguridad.md).

---

## 1. Reproducibilidad

- Todo el workspace usa `resolver = "3"`, edición 2024 y MSRV **1.99.1** (`[workspace.package]` en `Cargo.toml`).
- `Cargo.lock` se versiona y se instala con `--locked` en CI y en las rutas documentadas de compilación.
- Las dependencias se declaran en `[workspace.dependencies]` cuando son compartidas, para evitar versiones divergentes entre crates.
- Añadir una dependencia exige: necesidad real, revisión de licencia, revisión de mantenimiento, comprobar que no duplica una capacidad ya presente y ejecutar las pruebas y auditorías de este documento.
- No se aceptan dependencias con `git` como fuente ni registros alternativos: `deny.toml` rechaza ambos (`unknown-git = "deny"`, `unknown-registry = "deny"`, `allow-git = []`).

## 2. Licencias

`deny.toml` permite únicamente:

| Licencia | Uso esperado |
|---|---|
| `Apache-2.0` | Dependencias permisivas con concesión explícita de patentes |
| `MIT` | Dependencias permisivas |
| `Unicode-3.0` | Datos y tablas Unicode |

Reglas:

- Cualquier licencia fuera de esa lista bloquea la integración hasta revisión explícita.
- Las licencias se verifican con umbral de confianza `0.93`; una detección dudosa se trata como no permitida.
- Los crates privados del workspace no se auditan como licencias de terceros (`[licenses.private] ignore = true`).
- No se copia código de terceros sin conservar su aviso de licencia y sin revisar compatibilidad.

## 3. Vulnerabilidades y avisos

- `cargo audit` consulta la base pública [RustSec](https://rustsec.org/) contra `Cargo.lock`.
- `cargo deny check advisories` aplica la misma base con la política del repositorio; `unmaintained = "workspace"` reporta dependencias sin mantenimiento que afectan al workspace.
- Un aviso con severidad alta o crítica bloquea la entrega. La corrección preferida es actualizar o retirar la dependencia; si no es posible, se documenta la evaluación, el impacto real y una excepción temporal en `deny.toml`.
- Las excepciones deben indicar el identificador del aviso, la justificación y la fecha de revisión. No se añaden excepciones para obtener un resultado verde sin análisis.
- Un aviso sin parche disponible no se oculta: se registra como riesgo residual en el informe de seguridad.

Comandos locales:

```bash
cargo deny check
cargo audit
```

El recolector de validación ejecuta ambas comprobaciones como pasos opcionales: si la herramienta no está instalada, el informe marca **No evaluada**; en CI se instalan versiones fijadas y un fallo rompe el job.

## 4. Versiones de las herramientas

| Herramienta | Versión fijada | Dónde |
|---|---|---|
| Rust / Cargo | 1.99.1 | CI y MSRV |
| `cargo-deny` | Última estable instalada con `--locked` en CI | Job `dependencies` y `validation-report` |
| `cargo-audit` | 0.22.2 | Job `dependencies` y `validation-report` |
| `cargo-fuzz` | 0.13.2 | Job `fuzz-smoke` y `validation-report` |
| Nightly de fuzzing | `nightly-2026-10-16` | Job `fuzz-smoke` y `validation-report` |

Fijar la versión de `cargo-audit` evita que un cambio de formato o de base de datos altere el resultado sin revisión. `cargo-deny` se instala con `--locked`; su versión efectiva queda registrada en el log del job.

## 5. Cadencia de revisión

- **Cada push y pull request:** `cargo deny check` y `cargo audit` en CI.
- **Semanal:** el workflow programado ejecuta el recolector con 10 000 iteraciones de fuzzing y conserva el informe como artefacto.
- **Antes de publicar:** revisar avisos nuevos, licencias y `Cargo.lock`; la firma y el SBOM pertenecen a la subfase 12.9.
- **Al añadir o actualizar una dependencia:** ejecutar ambas auditorías y las pruebas del workspace en la misma revisión.

## 6. Límites

- `cargo audit` y `cargo-deny` dependen de que la base de avisos esté actualizada y de que el aviso exista; no detectan vulnerabilidades desconocidas ni código malicioso en una dependencia legítima.
- Las dependencias de compilación pueden ejecutar `build.rs` y macros procedurales cuando el usuario autoriza compilar. La auditoría no sustituye Workspace Trust ni un sandbox.
- `cargo-deny` no prueba que una licencia sea compatible con el uso comercial previsto; esa revisión es jurídica.
- El fuzzing y los límites de memoria/CPU se documentan en el [informe de seguridad](fase-12-seguridad.md) y en [`docs/testing.md`](testing.md).
