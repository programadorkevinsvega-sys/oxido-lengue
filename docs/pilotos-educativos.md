# Pilotos educativos: protocolo

Protocolo para evaluar el material educativo de Óxido con estudiantes reales. **No se ha ejecutado ningún piloto.** Este documento describe el diseño, los instrumentos y las condiciones para que un centro o docente pueda realizarlo y publicar resultados con limitaciones explícitas.

No se afirma que Óxido mejore el aprendizaje, reduzca el abandono ni sea más fácil que otro lenguaje. Un piloto pequeño solo produce evidencia local y descriptiva.

## Objetivos e hipótesis

**Objetivo principal:** comprobar si el [tutorial](tutorial.md) y los [ejercicios](ejercicios.md) permiten completar las tareas básicas sin ayuda constante del docente.

**Objetivos secundarios:**

- Identificar los diagnósticos y errores de instalación más frecuentes.
- Medir el tiempo hasta la primera ejecución correcta.
- Recoger la percepción de claridad del material.
- Detectar conceptos de Rust que el material no explica bien.

**Hipótesis de trabajo (a contrastar, no afirmadas):**

1. La mayoría del grupo piloto completa los ejercicios 1–3 en una sesión de 90 minutos con ayuda puntual.
2. Los errores más frecuentes son de instalación y de tipos, no de sintaxis de Óxido.
3. El material no empeora la comprensión de ownership frente al grupo de comparación.

## Diseño

- **Tipo:** piloto cuasiexperimental con grupo piloto y grupo de comparación cuando sea posible; si no, estudio de un solo grupo con pre y post prueba.
- **Asignación:** voluntaria y reversible. Si el centro lo permite, aleatoria por clase o por bloque horario; nunca por características personales.
- **Duración:** una sesión de instalación y tres sesiones de 90 minutos, o dos sesiones de 120 minutos.
- **Material:** el [tutorial](tutorial.md) y los [ejercicios](ejercicios.md) para el grupo piloto; un material de control equivalente en carga y objetivos, no necesariamente en lenguaje.
- **Cegamiento:** el docente que corrige no debe saber qué grupo es cuál cuando sea posible.
- **Requisito:** aprobación del centro, consentimiento informado y revisión de privacidad antes de recoger datos.

## Participantes

- Estudiantes de secundaria, formación profesional o universidad con conocimientos básicos de programación.
- Edad mínima según la legislación aplicable; si hay menores, consentimiento del tutor legal además del asentimiento del estudiante.
- Participación voluntaria, sin penalización académica por retirarse.
- Se excluyen datos de quienes se retiren antes del análisis; la retirada no requiere justificación.

## Materiales

- Un equipo por estudiante o por pareja con Rust 1.99.1 o posterior, `rustfmt` y la CLI instalada.
- El [tutorial](tutorial.md) y los [ejercicios](ejercicios.md) en la versión fijada del repositorio.
- Un repositorio o carpeta limpia por participante; sin dependencias externas para los ejercicios 1–3.
- Hoja de diagnóstico para registrar errores de instalación sin capturar rutas personales.
- Cronómetro o registro del tiempo por tarea, sin identificar a la persona.

## Instrumentos

### Pre-prueba (15 minutos)

1. Un programa mínimo en el lenguaje que ya conozcan: imprimir un saludo y sumar dos números.
2. Tres preguntas conceptuales sobre tipos, mutabilidad y errores de compilación.
3. Autoevaluación de confianza (escala 1–5) para leer un mensaje de error.

### Post-prueba (20 minutos)

1. Un ejercicio nuevo equivalente a los del nivel 1.
2. Las mismas tres preguntas conceptuales.
3. Autoevaluación de confianza y encuesta de claridad.

### Observación de sesión

- Hora de inicio y de primera ejecución correcta.
- Ejercicios completados.
- Diagnósticos `OX*` y de `rustc` observados, sin transcribir rutas.
- Ayudas del docente: tipo y frecuencia.
- Incidencias técnicas por plataforma.

### Encuesta de claridad (5 minutos)

Escala 1–5 y una pregunta abierta:

- El tutorial explica lo necesario para empezar.
- Los ejemplos se ejecutan como se describe.
- Los errores indican qué corregir.
- Los ejercicios son asequibles en el tiempo disponible.
- Qué cambiarías del material.

### Entrevista breve (opcional, 10 minutos)

A 3–5 estudiantes voluntarios, con consentimiento, para profundizar en dificultades. Se transcribe sin nombres ni datos identificables.

## Métricas

| Métrica | Cómo se calcula | Uso |
|---|---|---|
| Finalización | Ejercicios completados / ejercicios propuestos | Resultado principal |
| Tiempo hasta primera ejecución | Primera ejecución con código 0 menos inicio | Dificultad de instalación |
| Tiempo por ejercicio | Fin menos inicio de cada ejercicio | Carga del material |
| Diagnósticos por ejercicio | Conteo por código `OX*` o `rustc` | Puntos de fricción |
| Ayudas del docente | Conteo por tipo | Autonomía |
| Claridad percibida | Mediana de la escala | Calidad percibida |
| Cambio conceptual | Diferencia pre/post | Señal, no prueba causal |

No se calculan efectos causales con muestras pequeñas. Se reportan medianas, rangos y recuentos; se evita presentar un porcentaje sin denominador.

## Procedimiento

1. **Preparación (2 semanas antes).** Aprobación del centro, consentimiento, revisión de privacidad, fijación del commit y del toolchain, prueba de instalación en los equipos del aula.
2. **Sesión 0: instalación (45 minutos).** Verificar Rust, `rustfmt` y la CLI. Registrar incidencias por plataforma sin datos personales.
3. **Sesión 1: tutorial (90 minutos).** Lecciones 1 y 2, pre-prueba al inicio.
4. **Sesión 2: tutorial y ejercicios (90 minutos).** Lección 3 y ejercicios 1–2.
5. **Sesión 3: ejercicios y cierre (90 minutos).** Ejercicio 3, post-prueba y encuesta.
6. **Análisis (2 semanas después).** Limpieza, agregación, análisis descriptivo y redacción del informe.
7. **Devolución.** Informe al centro y a los participantes, con limitaciones y sin comparaciones individuales.

## Datos y privacidad

- **Minimización:** solo los datos necesarios para las métricas. Nada de nombres, correos, rutas absolutas, identificadores de equipo ni grabaciones sin consentimiento.
- **Identificadores:** cada participante recibe un código aleatorio; la tabla de correspondencia se guarda separada y se destruye al terminar el análisis.
- **Almacenamiento:** cifrado, acceso restringido al equipo investigador y sin sincronización a servicios no aprobados.
- **Retención:** datos agregados el tiempo necesario para el informe; datos crudos se destruyen según el plazo acordado con el centro.
- **Telemetría:** el proyecto no envía telemetría. Cualquier herramienta adicional debe declararse y justificarse.
- **Publicación:** solo datos agregados o citas anonimizadas. Si el centro lo exige, revisión previa del informe.
- **Menores:** consentimiento del tutor legal y asentimiento del estudiante; derecho a retirarse sin consecuencias.

## Análisis

- Estadística descriptiva: medianas, rangos, recuentos y porcentajes con denominador.
- Comparación pre/post por participante cuando exista emparejamiento.
- Comparación entre grupos solo si el diseño y el tamaño lo permiten; se reporta el tamaño muestral y la incertidumbre.
- Análisis temático de respuestas abiertas y entrevistas, con citas anonimizadas.
- Los resultados negativos o ambiguos se publican igual que los positivos.
- No se usa «estadísticamente significativo» sin la prueba y los supuestos correspondientes.

## Roles

| Rol | Responsabilidad |
|---|---|
| Docente | Imparte las sesiones, aplica instrumentos y registra observaciones |
| Coordinador del piloto | Aprobaciones, consentimiento, calendario y custodia de datos |
| Analista | Limpieza, análisis y redacción del informe |
| Revisor | Comprueba privacidad, limitaciones y que no se exageren conclusiones |
| Mantenedor de Óxido | Fija el commit y corrige fallos del material encontrados en el piloto |

Una persona puede asumir varios roles en un piloto pequeño, pero la revisión de privacidad debe ser independiente de quien imparte.

## Cronograma sugerido

| Semana | Actividad |
|---|---|
| −4 | Contacto con el centro, alcance y aprobaciones |
| −2 | Consentimiento, revisión de privacidad y prueba de instalación |
| 0 | Sesión de instalación |
| 1 | Sesión 1 |
| 2 | Sesión 2 |
| 3 | Sesión 3 y cierre de datos |
| 5 | Análisis y borrador del informe |
| 6 | Revisión y devolución al centro |

## Criterios de avance

Un piloto se considera **ejecutado** solo si existen:

- protocolo aprobado y consentimiento documentado;
- commit y toolchain fijados;
- datos recogidos y anonimizados;
- análisis con denominadores y limitaciones;
- revisión de privacidad;
- informe publicado con resultados, incluidos los negativos.

Un piloto **no** se considera completado por tener materiales o una sesión suelta. No se declara eficacia educativa a partir de una muestra pequeña.

## Plantilla de informe

```text
Título:
Fecha y commit:
Centro y contexto (sin datos personales):
Diseño y tamaño muestral:
Instrumentos aplicados:
Resultados principales (medianas, rangos, recuentos):
Diagnósticos más frecuentes:
Incidencias de instalación:
Limitaciones:
Datos no recogidos o perdidos:
Conclusiones (descriptivas, no causales):
Recomendaciones para el material:
Revisión de privacidad:
Anexos anonimizados:
```

## Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Muestra pequeña y sesgada | Reportar tamaño, contexto y límites; no generalizar |
| Instalación fallida en el aula | Prueba previa en los equipos; plan alternativo con binario o entorno preparado |
| Datos personales en diagnósticos | Reglas de captura, revisión y anonimización antes de compartir |
| Efecto del docente | Mismo docente y guion para ambos grupos cuando sea posible |
| Abandono o retirada | Análisis por intención de tratar cuando proceda; reportar retiradas |
| Expectativa de resultado positivo | Publicar también resultados negativos y ambigüedades |

## Anexo: consentimiento mínimo

Incluye, como mínimo:

- finalidad del piloto y qué se recoge;
- carácter voluntario y derecho a retirarse sin consecuencias;
- quién accede a los datos y durante cuánto tiempo;
- que no se recogen nombres ni rutas personales;
- que los resultados se publican agregados o anonimizados;
- contacto del responsable y del centro;
- firma del estudiante y, si es menor, del tutor legal.

## Estado

- **Propuesto:** todo el protocolo y los instrumentos.
- **No ejecutado:** no hay centro, participantes, datos ni resultados.
- **Siguiente paso:** un centro interesado debe adaptar el protocolo a su normativa y fijar un commit antes de la primera sesión.
