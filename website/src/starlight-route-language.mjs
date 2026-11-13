const englishPages = new Set(['/referencia/en/quickstart/'])

export function onRequest(context, next) {
  if (!englishPages.has(context.url.pathname)) return next()

  const route = context.locals.starlightRoute
  if (!route) throw new Error('Starlight route data is unavailable for the English quickstart')

  route.lang = 'en'
  route.entryMeta.lang = 'en'
  route.head = route.head.map((entry) => {
    if (entry.tag !== 'meta' || entry.attrs?.property !== 'og:locale') return entry
    return { ...entry, attrs: { ...entry.attrs, content: 'en' } }
  })

  return next()
}
