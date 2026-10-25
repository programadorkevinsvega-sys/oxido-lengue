# Reglas Estrictas de Arquitectura — Monolito Modular (Rust)
## DDD + Clean Architecture + Hexagonal + SOLID

## 1. Alcance y autoridad

Este contrato define la arquitectura de Óxido, un lenguaje y toolchain que analiza `.ox`, genera Rust estándar y delega la compilación a Cargo y `rustc`. No describe un backend de negocio. `plan` define la especificación del lenguaje; este documento define cómo organizar su implementación. Si una decisión no se adapta al compilador, CLI o LSP, no se añade infraestructura para imitar una API.

Las reglas se aplican desde el diseño. No se crean paquetes, capas, traits o servicios vacíos para aparentar modularidad. Antes de crear un crate, debe tener responsabilidad estable, API identificable, dependencias justificadas y una razón de aislamiento comprobable.

## 2. Decisión arquitectónica

Óxido se implementa como un **monolito modular Rust** distribuido en crates internos dentro de un solo Cargo workspace. CLI y LSP son interfaces de entrada que componen los mismos casos de uso. El compilador no se divide en microservicios.

El bounded context de negocio/técnico es **la toolchain del lenguaje Óxido**. Lexer, AST, parser, diagnósticos, generación Rust y ejecución mediante Cargo son módulos del pipeline; no se fuerzan como bounded contexts de negocio independientes.

La comunicación entre módulos dentro del proceso usa APIs Rust tipadas. No se introducen gRPC, NATS, HTTP, bases de datos, Redis, BFF, tenants, pagos o eventos distribuidos: no pertenecen al producto definido.

## 3. Capas y dirección de dependencias

```text
Interfaces de entrada                 Adaptadores de salida
CLI ───────┐                           Generador de Rust
            ├──> Aplicación/Compiler ──> Ejecutor Cargo/rustc
LSP ───────┘             │
                         ▼
                  Dominio del lenguaje
              Source · Syntax · AST · Diagnostics
```

Dirección obligatoria:

1. **Dominio:** modelos de fuente, tokens, AST y diagnósticos; reglas e invariantes del lenguaje. No depende de Cargo, CLI, LSP, filesystem concreto ni generadores externos.
2. **Aplicación:** casos de uso como analizar, verificar, compilar, formatear, importar y exportar. Orquesta el dominio y define los puertos necesarios. No contiene lógica de transporte ni llama directamente a procesos del sistema.
3. **Puertos:** contratos pequeños y tipados para generación Rust, sistema de archivos/configuración y ejecución de Cargo cuando estos límites necesiten sustituibilidad o aislamiento.
4. **Adaptadores de salida:** backend Rust implementa generación mediante tokens/AST; adapter Cargo invoca procesos reales con argumentos separados y maneja sus diagnósticos JSON.
5. **Adaptadores de entrada:** CLI y LSP traducen comandos/protocolo a casos de uso y formatean resultados. No duplican lexer, parser ni reglas del lenguaje.
6. **Composición:** cada binario conecta implementaciones reales de los puertos. No hay contenedor global de dependencias ni service locator sin necesidad demostrada.

Las dependencias de código apuntan hacia el dominio y los contratos de aplicación. El dominio nunca importa adapters. Los adapters pueden depender de los contratos internos que implementan. No hay ciclos entre crates.

## 4. Estructura de carpetas acordada

La estructura física inicial queda preparada así:

```text
crates/
├── oxido-language/
│   └── src/domain/
│       ├── source/          # texto fuente, spans, posiciones
│       ├── syntax/          # tokens, lexer y reglas sintácticas
│       ├── ast/             # representación interna
│       ├── diagnostics/     # códigos, severidad, etiquetas
│       └── compatibility/   # capacidades y límites verificados
├── oxido-compiler/
│   └── src/
│       ├── application/    # casos de uso y coordinación
│       └── ports/          # interfaces de salida justificadas
├── oxido-rust-backend/     # adapter de generación Rust
├── oxido-cargo/            # adapter Cargo/rustc y JSON diagnostics
├── oxido-cli/              # entrada CLI y composition root
└── oxido-lsp/              # entrada LSP que reutiliza compiler

editors/
└── vscode-oxido/           # cliente/editor separado

website/                    # portal documental Markdown (Astro Starlight)

tests/
├── fixtures/
│   ├── valid/
│   └── invalid/
├── integration/
└── compatibility/

examples/                    # solo ejemplos que compilen
```

Los crates `oxido-language`, `oxido-rust-backend`, `oxido-compiler` y `oxido-cli` se materializaron en Fase 2; `oxido-language` incorporó el lexer en Fase 3. `oxido-cargo` y `oxido-lsp` continúan como límites previstos, no crates funcionales. No se añade un crate hasta que su API y función sean reales.

El portal `website/` es Astro Starlight estático, separado de la extensión de VS Code y del compilador. Los Markdown técnicos editables viven en `docs/`; el build prepara copias con metadatos y enlaces web. No mantengas versiones manuales duplicadas ni añadas backend, base de datos o telemetría para publicar documentación.

## 5. DDD aplicado sin forzarlo

- Mantén un lenguaje ubicuo para fuente, token, span, AST, diagnóstico, modo sintáctico, compilación y compatibilidad.
- Protege invariantes de AST, rangos, configuración y estado de análisis mediante constructores y tipos adecuados.
- Usa tipos de valor para identificadores/códigos/rangos cuando prevengan errores reales.
- No inventes agregados, repositorios, eventos de dominio, persistencia ni ACL: Óxido no tiene estado de negocio persistente ni contextos de negocio distribuidos.
- Separa la representación interna del protocolo LSP y de los formatos de salida CLI.

## 6. Clean Architecture y Hexagonal

- El dominio no conoce `syn`, `quote`, `proc_macro2`, Cargo, `rustc`, JSON-RPC ni VS Code; cuando una biblioteca sea esencial para representar sintaxis, se confina al módulo que la use.
- Los casos de uso reciben datos explícitos y devuelven resultados tipados; no leen variables globales ni imprimen directamente.
- Los puertos se crean para límites externos reales (procesos, generación, filesystem/configuración), no para cada función.
- Los adapters traducen sus errores al modelo de diagnóstico sin perder causa, ubicación o código del compilador externo.
- CLI y LSP pueden compartir casos de uso, pero cada uno conserva sus responsabilidades de entrada/salida.

## 7. SOLID y Rust

- **SRP:** un módulo o tipo tiene una responsabilidad coherente dentro del pipeline.
- **OCP:** extender alias, diagnósticos o adapters no debe requerir reescribir etapas no relacionadas; no usar abstracciones especulativas.
- **LSP:** implementaciones de puertos respetan el contrato de resultados, errores y cancelación.
- **ISP:** interfaces pequeñas alineadas con casos de uso reales.
- **DIP:** aplicación depende de contratos propios; infraestructura implementa esos contratos.
- Prefiere ownership, enums, generics y dispatch estático donde alcancen; `dyn Trait`/`Arc` solo si la composición dinámica está justificada.

## 8. Reglas de código y seguridad

- Un archivo debe tener una responsabilidad comprensible. Usa 200 líneas como señal para revisar, no como mandato de fragmentación mecánica; gramáticas, tablas y generación pueden justificar excepciones documentadas.
- No uses `unsafe` en el compilador salvo excepción justificada y revisada; el valor por defecto es `#![forbid(unsafe_code)]`.
- El backend emite Rust mediante tokens/AST, no interpolación textual de input.
- La integración de procesos usa `Command` con argumentos separados, rutas canonicalizadas y acceso restringido al workspace.
- Trata `.ox`, `oxido.toml`, código incompleto, dependencias y mensajes LSP como entradas no confiables. Limita recursos y evita pánicos.
- No compiles workspaces no confiables automáticamente. La extensión respeta Workspace Trust.
- No se envía telemetría por defecto; no se registran fuentes completas, secretos ni rutas absolutas en reportes exportados.

## 9. Calidad, pruebas y rendimiento

- Prueba cada límite real: lexer/parser, backend, adapter Cargo, CLI y protocolo LSP.
- Dobles de prueba pueden aislar unidades; no sustituyen Cargo/`rustc` en el flujo end-to-end.
- Mantén casos positivos y negativos, regresiones, pruebas de propiedades y fuzzing donde aporten valor.
- Mide tiempo y memoria de lexer, parser, generación, compilación y LSP antes de optimizar. Usa benchmarks reproducibles y no inventes SLO.
- Métricas remotas y Prometheus no son requisitos de la herramienta local. Solo se reevalúan si se construye un servicio compartido de compilación/playground.
- Versiona lockfile para aplicaciones, usa `--locked` en CI y audita dependencias/licencias según la etapa del proyecto.

## 10. Criterios para añadir un crate

Antes de materializar cada directorio como crate, documenta:

1. Responsabilidad y propietario.
2. API pública mínima.
3. Dependencias permitidas y dirección de capas.
4. Motivo por el que debe ser crate separado y no módulo interno.
5. Pruebas de su contrato.

Si la separación solo añade manifests y re-exports sin aislamiento útil, conserva el componente como módulo interno. La arquitectura se revisa conforme crezca el compilador; no se adelantan límites de microservicios.
