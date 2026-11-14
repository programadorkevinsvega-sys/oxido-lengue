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
      routeMiddleware: ['./src/starlight-route-language.mjs'],
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
          label: 'Guías',
          items: [
            { label: 'Instalación', link: '/referencia/instalacion/' },
            { label: 'Contribuir al proyecto', link: '/referencia/contribuir/' },
            { label: 'Tutorial progresivo', link: '/referencia/tutorial/' },
            { label: 'Ejercicios', link: '/referencia/ejercicios/' },
            { label: 'Educación', link: '/referencia/education/' },
            { label: 'Pilotos educativos', link: '/referencia/pilotos-educativos/' },
            { label: 'English quickstart', link: '/referencia/en/quickstart/' },
            { label: 'VS Code y LSP', link: '/referencia/vscode/' },
            { label: 'Preparación empresarial', link: '/referencia/enterprise-readiness/' },
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
          ],
        },
      ],
    }),
  ],
})
