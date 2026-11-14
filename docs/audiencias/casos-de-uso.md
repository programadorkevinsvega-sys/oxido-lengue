# Casos de uso previstos

Estos casos guían el diseño del lenguaje y el material del sitio. El estado distingue entre flujos probados y capacidades que siguen pendientes.

| Audiencia | Caso | Estado actual |
|---|---|---|
| Desarrolladores | Tokenizar y diagnosticar sintaxis `.ox` | Implementado como biblioteca; alcance descrito en la [especificación](../language-spec.md) y el [pipeline](../compiler-pipeline.md) |
| Desarrolladores | Analizar, generar y compilar `.ox` como Rust | CLI: `check`/`verificar`, `build`/`compilar`, `run`/`ejecutar`, `test`/`probar`, exportar/importar, `nuevo` y `formato`; límites en la [matriz](../compatibility.md) |
| Empresas | Evaluar límites, seguridad y reproducibilidad | Reproducibilidad y controles de CLI documentados; el producto no está listo para producción y el código de dependencias no está sandboxeado |
| Estudiantes | Aprender sintaxis bilingüe y relacionarla con Rust | [Tutorial](../tutorial.md) y [ejercicios](../ejercicios.md) ejecutables; la cobertura sigue limitada a la matriz |
| Docentes | Diseñar ejercicios verificables y progresivos | Material inicial con ejemplos verificados y ejercicios propuestos; sin validación en aula, ver [Educación](../education.md) |
| Empresas | Evaluar controles antes de un piloto | Lista de evaluación y riesgos residuales en [Preparación empresarial](../enterprise-readiness.md) |
| Mantenedores | Contribuir lexer, parser, CLI y documentación | [Guía para clonar, compilar y contribuir](../contribuir.md) y [arquitectura](../architecture.md) |

Los casos se actualizarán cuando cambie el estado demostrado por pruebas.
