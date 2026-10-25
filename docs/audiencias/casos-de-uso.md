# Casos de uso previstos

Estos casos guían el diseño del lenguaje y el material del sitio. El estado distingue entre flujos probados y capacidades que siguen pendientes.

| Audiencia | Caso | Estado actual |
|---|---|---|
| Desarrolladores | Tokenizar y diagnosticar sintaxis `.ox` | Implementado como biblioteca; cobertura descrita en [Fase 3](/referencia/fase-3-lexer/) |
| Desarrolladores | Analizar, generar y compilar `.ox` como Rust | Probado con Hola, mundo y ejemplos de Fase 7; el resto se detalla en la matriz |
| Empresas | Evaluar límites, seguridad y reproducibilidad | Documentado parcialmente; controles empresariales completos pendientes |
| Estudiantes | Aprender sintaxis bilingüe y relacionarla con Rust | Ejemplos ejecutables de bloques, estructuras iniciales e interoperabilidad Rust; características futuras marcadas como pendientes |
| Docentes | Diseñar ejercicios verificables y progresivos | Propuesto para la fase de educación |
| Mantenedores | Contribuir lexer, parser, CLI y documentación | Roadmap y arquitectura disponibles |

Los casos se actualizarán cuando cambie el estado demostrado por pruebas.
