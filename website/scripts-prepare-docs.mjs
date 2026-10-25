import { cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const sourceDocs = resolve(root, '../docs')
const contentDocs = join(root, 'src/content/docs')
const references = join(contentDocs, 'referencia')
const audiences = join(contentDocs, 'audiencias')

await rm(references, { recursive: true, force: true })
await rm(audiences, { recursive: true, force: true })
await mkdir(references, { recursive: true })
await mkdir(audiences, { recursive: true })

async function addFrontmatter(source, destination, fallbackTitle) {
  const body = await readFile(source, 'utf8')
  if (body.startsWith('---\n')) {
    await cp(source, destination)
    return
  }
  const title = body.match(/^#\s+(.+)$/m)?.[1]?.trim() ?? fallbackTitle
  const description = `Documentación de Óxido: ${title}.`
  const rewritten = body
    .replaceAll('```ox', '```text')
    .replaceAll('```ebnf', '```text')
    .replaceAll('../ARCHITECTURE_RULES.md', '/referencia/reglas-de-arquitectura/')
    .replaceAll('../website/README.md', '/referencia/sitio-documental/')
    .replace(/\]\(([^/][^)]*?)\.md\)/g, '](/referencia/$1/)')
    .replaceAll('](/referencia/architecture/)', '](/referencia/arquitectura/)')
  const content = `---\ntitle: ${JSON.stringify(title)}\ndescription: ${JSON.stringify(description)}\n---\n\n${rewritten}`
  await mkdir(dirname(destination), { recursive: true })
  await writeFile(destination, content)
}

for (const entry of await readdir(sourceDocs, { withFileTypes: true })) {
  if (!entry.isFile() || !entry.name.endsWith('.md') || entry.name === 'index.md') continue
  const source = join(sourceDocs, entry.name)
  const destinationName = entry.name === 'architecture.md' ? 'arquitectura.md' : entry.name
  await addFrontmatter(source, join(references, destinationName), entry.name)
}

for (const entry of await readdir(join(sourceDocs, 'audiencias'), { withFileTypes: true })) {
  if (!entry.isFile() || !entry.name.endsWith('.md')) continue
  await addFrontmatter(
    join(sourceDocs, 'audiencias', entry.name),
    join(audiences, entry.name),
    entry.name,
  )
}

await addFrontmatter(
  resolve(root, '../ARCHITECTURE_RULES.md'),
  join(references, 'reglas-de-arquitectura.md'),
  'Reglas de arquitectura',
)
await addFrontmatter(
  resolve(root, 'README.md'),
  join(references, 'sitio-documental.md'),
  'Sitio documental',
)
