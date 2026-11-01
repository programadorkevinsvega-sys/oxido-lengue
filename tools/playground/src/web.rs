pub const PAGE: &str = r##"<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Playground de Óxido</title>
<style>
:root { color-scheme: light dark; font-family: ui-sans-serif, system-ui, sans-serif; }
body { margin: 0; background: #101418; color: #e6edf3; }
main { max-width: 1100px; margin: 0 auto; padding: 1.5rem; }
h1 { font-size: 1.4rem; margin: 0 0 .25rem; }
p.limites { margin: 0 0 1rem; color: #9fb0c0; font-size: .9rem; }
.workspace { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 1rem; }
@media (max-width: 800px) { .workspace { grid-template-columns: 1fr; } }
label { display: block; font-weight: 600; margin-bottom: .35rem; }
textarea, pre { width: 100%; box-sizing: border-box; border-radius: .5rem; border: 1px solid #2d3742; background: #0b0f13; color: #e6edf3; font: .92rem/1.5 ui-monospace, SFMono-Regular, Consolas, monospace; }
textarea { min-height: 26rem; padding: .75rem; resize: vertical; }
pre { min-height: 13rem; padding: .75rem; overflow: auto; margin: 0; white-space: pre-wrap; }
button { margin: 1rem 0; padding: .6rem 1.2rem; border-radius: .4rem; border: 0; background: #2f81f7; color: #fff; font-weight: 600; cursor: pointer; }
button:disabled { opacity: .6; cursor: wait; }
.estado { color: #9fb0c0; margin-left: .75rem; font-size: .9rem; }
.salida { background: #0b0f13; border: 1px solid #2d3742; border-radius: .5rem; }
.salida h2 { font-size: .95rem; margin: 0; padding: .5rem .75rem; border-bottom: 1px solid #2d3742; }
.salida pre { border: 0; min-height: 8rem; }
</style>
</head>
<body>
<main>
<h1>Playground de Óxido</h1>
<p class="limites">Ejecución aislada sin red. Límites: 64 KiB de fuente, 12 s de pared, 2 GiB de memoria, 256 procesos y 64 KiB por salida. Solo biblioteca estándar y dependencias ya disponibles.</p>
<div class="workspace">
  <section>
    <label for="fuente">Código</label>
    <textarea id="fuente" spellcheck="false">sintaxis indentacion

funcion main():
    dejar nombre = "Óxido"
    println!("Hola, {nombre}")
</textarea>
    <button id="ejecutar" type="button">Ejecutar</button>
    <span class="estado" id="estado" role="status"></span>
  </section>
  <section class="salida" aria-live="polite">
    <h2>Salida</h2>
    <pre id="salida"></pre>
    <h2>Diagnósticos</h2>
    <pre id="errores"></pre>
  </section>
</div>
</main>
<script>
const fuente = document.getElementById('fuente');
const boton = document.getElementById('ejecutar');
const estado = document.getElementById('estado');
const salida = document.getElementById('salida');
const errores = document.getElementById('errores');

async function ejecutar() {
  boton.disabled = true;
  estado.textContent = 'Ejecutando…';
  salida.textContent = '';
  errores.textContent = '';
  try {
    const response = await fetch('/api/ejecutar', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ fuente: fuente.value })
    });
    const data = await response.json();
    if (!response.ok) {
      throw new Error(data.error || 'La solicitud falló');
    }
    salida.textContent = data.salida || '(sin salida)';
    errores.textContent = data.errores || '(sin diagnósticos)';
    estado.textContent = data.tiempo_agotado
      ? 'Tiempo agotado'
      : `Código ${data.codigo ?? 'desconocido'} · ${data.duracion_ms} ms${data.truncado ? ' · salida truncada' : ''}`;
  } catch (error) {
    errores.textContent = String(error);
    estado.textContent = 'Error';
  } finally {
    boton.disabled = false;
  }
}

boton.addEventListener('click', ejecutar);
fuente.addEventListener('keydown', (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault();
    ejecutar();
  }
});
</script>
</body>
</html>
"##;
