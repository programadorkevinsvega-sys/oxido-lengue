import { defineConfig } from 'astro/config'
import starlight from '@astrojs/starlight'

export default defineConfig({
  site: 'https://oxido-lengue.pages.dev',
  integrations: [
    starlight({
      title: 'Óxido',
      description: 'Documentación del lenguaje Óxido para desarrolladores, empresas y estudiantes.',
      disable404Route: true,
      customCss: ['./src/styles/custom.css'],
      defaultLocale: 'es',
      locales: {
        root: { label: 'Español', lang: 'es' },
      },
      sidebar: [
        {
          label: 'Para cada audiencia',
          items: [
            { label: 'Desarrolladores', link: '/audiencias/desarrolladores/' },
            { label: 'Empresas', link: '/audiencias/empresas/' },
            { label: 'Estudiantes', link: '/audiencias/estudiantes/' },
            { label: 'Casos de uso', link: '/audiencias/casos-de-uso/' },
          ],
        },
        {
          label: 'Lenguaje',
          items: [
            { label: 'Especificación', link: '/referencia/language-spec/' },
            { label: 'Gramática', link: '/referencia/grammar/' },
            { label: 'Modos', link: '/referencia/syntax-modes/' },
            { label: 'Alias', link: '/referencia/aliases/' },
            { label: 'Compatibilidad', link: '/referencia/compatibility/' },
            { label: 'Diagnósticos', link: '/referencia/diagnostics/' },
          ],
        },
        {
          label: 'Proyecto',
          items: [
            { label: 'Arquitectura', link: '/referencia/arquitectura/' },
            { label: 'Pipeline', link: '/referencia/compiler-pipeline/' },
            { label: 'Roadmap', link: '/referencia/roadmap/' },
            { label: 'Decisiones de arquitectura (ADR)', link: '/referencia/adr/' },
          ],
        },
      ],
    }),
  ],
})
