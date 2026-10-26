# ADR-0009: Evidencia reproducible y alcance de benchmarks

- **Estado:** Aceptada
- **Fecha:** 2026-10-26
- **Fase:** 10, laboratorio de validación

## Contexto

Una característica documentada, una campaña de fuzzing y una medición de rendimiento requieren evidencias distintas. Los conteos de fuzz no prueban ausencia de defectos; un benchmark aislado de una API tampoco demuestra rendimiento general del compilador o del lenguaje. Los informes también pueden filtrar rutas locales si guardan logs sin revisar.

## Decisión

1. Cada capacidad anunciada enlaza pruebas identificables; los informes separan aprobado, fallido y no evaluado, y registran commit, versiones, entorno y comandos relevantes.
2. La CI conserva como artefactos versionados por commit los informes que necesiten inspección posterior; los informes evitan rutas absolutas, secretos y logs crudos innecesarios.
3. Fuzzing se reporta por harness, corpus, versión, límites e iteraciones. El número de iteraciones no se presenta como cobertura ni como garantía de ausencia de defectos.
4. Un benchmark es una evidencia separada de las pruebas funcionales. Debe describir carga, repeticiones, perfil de compilación, calentamiento, métricas y entorno, y declarar explícitamente los errores y la variabilidad.
5. Las comparaciones Óxido/Rust usan el mismo cliente, dependencias, datos y configuración, ejecutan los servidores secuencialmente y compilan ambos en el mismo perfil. Sus resultados solo caracterizan esa carga y ese entorno; no justifican afirmaciones de rendimiento de producción ni del compilador sin medición específica.
6. No se inventan objetivos ni se extrapolan resultados. Si no se ejecutó una medición, se informa como no medida.

## Consecuencias

- La matriz de compatibilidad se apoya en regresiones reproducibles y no en descripciones aspiracionales.
- Los informes de fuzz, conformidad y rendimiento son artefactos distintos aunque se publiquen en un mismo workflow.
- Los benchmarks de servicios se consideran exploratorios, sensibles al host y no representativos de productividad o rendimiento general del lenguaje.
- Los cambios de CI que alteren evidencias requieren actualizar el informe propietario y conservar los resultados de CI reales.

## Evidencia

El laboratorio, sus suites y límites están en [Fase 10](../fase-10-laboratorio-validacion.md) y [testing.md](../testing.md). El [informe de campaña fuzz](../reports/fase-10/2026-10-26-10000-por-target.md) y el [benchmark release](../reports/fase-10/2026-10-26-benchmark-release-github.md) muestran reportes separados según este contrato.
