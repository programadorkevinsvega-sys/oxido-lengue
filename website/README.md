# Sitio documental de Óxido

Sitio estático en Astro Starlight. Las páginas de audiencia y referencia se escriben en Markdown. Los documentos técnicos canónicos permanecen en `../docs`; `scripts-prepare-docs.mjs` los copia al contenido de Starlight durante el build y les agrega metadatos sin modificar los originales.

## Desarrollo local

Requiere Node.js 22.23.4 o superior; `.nvmrc` fija la versión recomendada para desarrollo local.

```sh
npm ci
npm run dev
```

El build ejecuta la preparación de contenido antes de generar el sitio estático:

```sh
npm run build
npm run preview
```

## Cloudflare Pages

Configura el directorio raíz del proyecto como `website`, el comando de build como `npm run build` y el directorio de salida como `dist`. Pages puede generar previews para cambios en ramas y pull requests. No hace falta backend ni runtime dinámico para esta documentación.

El portal ya está publicado en Cloudflare Pages. En el proyecto de Pages, conserva la raíz `website`, el build `npm run build` y la salida `dist`; no configures `npx wrangler deploy`, que pertenece al flujo de Workers y busca una configuración distinta.

## Contenido y estado

- `src/content/docs/index.mdx`: portada del portal.
- `src/pages/404.astro`: página personalizada para rutas no encontradas, emitida como `404.html` para Cloudflare Pages.
- `../docs/audiencias/*.md`: páginas fuente para desarrolladores, empresas y estudiantes.
- `../docs/*.md`: especificaciones, roadmap e informes técnicos que se publican como referencia.
- El script genera copias bajo `src/content/docs/referencia/` y `audiencias/`; son artefactos locales y no se versionan.
- Los ejemplos deben distinguir sintaxis propuesta de características implementadas. La rebanada inicial compila `examples/hola_mundo.ox`; no se afirma compatibilidad general.

La propiedad `site` de `astro.config.mjs` usa el dominio público `https://oxido-lengue.pages.dev`; Astro genera el sitemap en `dist/sitemap-index.xml`. La página personalizada `src/pages/404.astro` se emite como `dist/404.html`, que Cloudflare Pages sirve para rutas inexistentes. Se desactiva la ruta 404 interna de Starlight para evitar que compita con esta página estática.
