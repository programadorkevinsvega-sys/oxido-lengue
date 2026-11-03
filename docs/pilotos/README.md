# Kit para ejecutar un piloto educativo

Este directorio contiene las plantillas para ejecutar el piloto descrito en [Pilotos educativos](../pilotos-educativos.md). **Todavía no se ha ejecutado ningún piloto y no hay resultados reales.**

El kit no sustituye la aprobación del centro, la revisión de privacidad ni el consentimiento. Tampoco convierte una sesión aislada en evidencia de eficacia educativa.

## Archivos

| Archivo | Uso |
|---|---|
| [`consentimiento.md`](consentimiento.md) | Plantilla de consentimiento informado y asentimiento |
| [`pre-post.md`](pre-post.md) | Pre-prueba, post-prueba y encuesta de claridad |
| [`observacion.md`](observacion.md) | Registro de sesión y hoja de incidencias |
| [`participantes-ejemplo.csv`](participantes-ejemplo.csv) | Datos **sintéticos** para probar el análisis |
| [`ejercicios-ejemplo.csv`](ejercicios-ejemplo.csv) | Datos **sintéticos** para probar el análisis |

## Antes de la primera sesión

1. Un centro o docente responsable aprueba el piloto y designa a la persona coordinadora.
2. Se revisa la normativa aplicable, el consentimiento y la retención de datos.
3. Se fija un commit del repositorio y la versión de Rust, `rustfmt` y la CLI.
4. Se prueba la instalación en los equipos del aula.
5. Se asigna un código anónimo a cada participante (`P-001` para el grupo piloto, `C-001` para el control). La tabla que relaciona códigos con personas se guarda separada y se destruye al terminar el análisis.

## Durante el piloto

- Rellena una fila por participante en `participantes.csv`.
- Rellena una fila por participante y ejercicio en `ejercicios.csv`.
- No escribas nombres, correos, rutas, identificadores de equipo ni texto libre en los CSV. El analizador rechaza esos datos.
- Usa la hoja de observación para notas cualitativas; tampoco debe contener datos personales.

Columnas de `participantes.csv`:

```text
codigo,grupo,tiempo_primera_ejecucion_seg,claridad,pre_puntos,post_puntos
```

Columnas de `ejercicios.csv`:

```text
codigo,ejercicio,completado,segundos,diagnosticos,ayudas
```

`diagnosticos` admite códigos `OX####`, `E####` o `W####` separados por `|`. `ayudas` es un número entero.

## Análisis

Con datos reales, desde la raíz del repositorio:

```sh
python3 tools/pilot_report.py \
  --participantes ruta/participantes.csv \
  --ejercicios ruta/ejercicios.csv \
  --salida target/informe-piloto.md
```

El informe incluye denominadores, medianas, rangos, diagnósticos y límites. No calcula significación estadística ni efectos causales. Un resultado negativo se publica igual que uno positivo.

Para comprobar el analizador con los datos sintéticos:

```sh
python3 tools/pilot_report.py \
  --participantes docs/pilotos/participantes-ejemplo.csv \
  --ejercicios docs/pilotos/ejercicios-ejemplo.csv \
  --salida target/informe-piloto-ejemplo.md \
  --sintetico
python3 -m unittest tools/test_pilot_report.py
```

El informe de ejemplo queda marcado como sintético y no debe citarse como resultado.

## Criterio para declarar el piloto ejecutado

Solo se declara ejecutado si existen protocolo aprobado, consentimiento documentado, commit y toolchain fijados, datos anonimizados, análisis con denominadores y limitaciones, revisión de privacidad e informe publicado. Tener el kit o hacer una sesión suelta no basta.
