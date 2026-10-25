# Sitio documental de Óxido

Sitio estático en Astro Starlight. Las páginas de audiencia y referencia se escriben en Markdown. Los documentos técnicos canónicos permanecen en `../docs`; `scripts-prepare-docs.mjs` los copia al contenido de Starlight durante el build y les agrega metadatos sin modificar los originales.

## Desarrollo local

Requiere Node.js 22.23.4 (fijado en `.node-version` y `.nvmrc`).

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

La primera publicación debe hacerse cuando se configure el proyecto de Pages con la cuenta/dominio del mantenedor. El build local se verifica de forma independiente.

## Contenido y estado

- `src/content/docs/index.mdx`: portada del portal.
- `../docs/audiencias/*.md`: páginas fuente para desarrolladores, empresas y estudiantes.
- `../docs/*.md`: especificaciones, roadmap e informes técnicos que se publican como referencia.
- El script genera copias bajo `src/content/docs/referencia/` y `audiencias/`; son artefactos locales y no se versionan.
- Los ejemplos deben distinguir sintaxis propuesta de características implementadas. No se afirma que `.ox` compile mientras Fase 5 siga pendiente.

El build local genera el HTML aunque el dominio aún no esté decidido. En ese caso Astro omite el sitemap hasta configurar `site` en `astro.config.mjs`; Starlight también puede mostrar un aviso interno al buscar metadatos para su ruta 404, sin impedir la salida estática.
