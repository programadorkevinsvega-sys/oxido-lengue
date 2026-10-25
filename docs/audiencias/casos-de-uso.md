# Casos de uso previstos

Estos casos guían el diseño del lenguaje y el material del sitio. No implican que el compilador ya los soporte.

| Audiencia | Caso | Estado actual |
|---|---|---|
| Desarrolladores | Tokenizar y diagnosticar sintaxis `.ox` | Implementado como biblioteca; cobertura descrita en [Fase 3](/referencia/fase-3-lexer/) |
| Desarrolladores | Analizar, generar y compilar `.ox` como Rust | Rebanada inicial verificada con `hola_mundo.ox`; el resto del lenguaje es parcial o pendiente |
| Empresas | Evaluar límites, seguridad y reproducibilidad | Documentado parcialmente; controles empresariales completos pendientes |
| Estudiantes | Aprender sintaxis bilingüe y relacionarla con Rust | Material inicial; `hola_mundo.ox` es ejecutable y los demás ejemplos deben indicar su estado |
| Docentes | Diseñar ejercicios verificables y progresivos | Propuesto para la fase de educación |
| Mantenedores | Contribuir lexer, parser, CLI y documentación | Roadmap y arquitectura disponibles |

Los casos se actualizarán cuando cambie el estado demostrado por pruebas.
