import { cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { existsSync, statSync } from 'node:fs'
import { dirname, join, posix, relative as relativePath, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(root, '..')
const sourceDocs = join(repoRoot, 'docs')
const contentDocs = join(root, 'src/content/docs')
const references = join(contentDocs, 'referencia')
const audiences = join(contentDocs, 'audiencias')
const repoBase = 'https://github.com/programadorkevinsvega-sys/oxido-lengue'

await rm(references, { recursive: true, force: true })
await rm(audiences, { recursive: true, force: true })
await mkdir(references, { recursive: true })
await mkdir(audiences, { recursive: true })

function rewriteLink(target, sourceDirectory) {
  const trimmed = target.trim()
  if (!trimmed || trimmed.startsWith('/') || /^(https?:|mailto:|tel:)/.test(trimmed)) {
    return target
  }
  const boundary = trimmed.search(/[?#]/)
  const path = boundary === -1 ? trimmed : trimmed.slice(0, boundary)
  const suffix = boundary === -1 ? '' : trimmed.slice(boundary)
  if (!path) return target

  const normalized = posix.normalize(posix.join(sourceDirectory, path))
  if (normalized.startsWith('../')) {
    const repoPath = posix.normalize(posix.join('docs', normalized))
    const absolute = join(repoRoot, repoPath)
    if (!existsSync(absolute)) return target
    const kind = statSync(absolute).isDirectory() ? 'tree' : 'blob'
    return `${repoBase}/${kind}/main/${repoPath}${suffix}`
  }
  if (path.endsWith('.md')) {
    return `/referencia/${normalized.replace(/\.md$/, '')}/${suffix}`
  }
  if (!posix.extname(path)) {
    return `/referencia/${normalized}/${suffix}`
  }
  return target
}

async function addFrontmatter(source, destination, fallbackTitle) {
  const body = await readFile(source, 'utf8')
  if (body.startsWith('---\n')) {
    await cp(source, destination)
    return
  }
  const title = body.match(/^#\s+(.+)$/m)?.[1]?.trim() ?? fallbackTitle
  const description = `Documentación de Óxido: ${title}.`
  const sourceDirectory = relativePath(sourceDocs, dirname(source)).split('\\').join('/')
  const rewritten = body
    .replaceAll('```ox', '```text')
    .replaceAll('```ebnf', '```text')
    .replaceAll('../ARCHITECTURE_RULES.md', '/referencia/reglas-de-arquitectura/')
    .replaceAll('../website/README.md', '/referencia/sitio-documental/')
    .replace(/\]\(([^)]+)\)/g, (match, target) => {
      if (match.startsWith('![')) return match
      return `](${rewriteLink(target, sourceDirectory)})`
    })
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

async function addMarkdownTree(sourceDirectory, destinationDirectory) {
  await mkdir(destinationDirectory, { recursive: true })
  for (const entry of await readdir(sourceDirectory, { withFileTypes: true })) {
    const source = join(sourceDirectory, entry.name)
    const destination = join(destinationDirectory, entry.name)
    if (entry.isDirectory()) {
      await addMarkdownTree(source, destination)
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      await addFrontmatter(source, destination, entry.name)
    }
  }
}

const reportsDirectory = join(sourceDocs, 'reports')
await addMarkdownTree(reportsDirectory, join(references, 'reports'))
const adrDirectory = join(sourceDocs, 'adr')
await addMarkdownTree(adrDirectory, join(references, 'adr'))
const promptsDirectory = join(sourceDocs, 'prompts')
await addMarkdownTree(promptsDirectory, join(references, 'prompts'))

await addFrontmatter(
  resolve(repoRoot, 'ARCHITECTURE_RULES.md'),
  join(references, 'reglas-de-arquitectura.md'),
  'Reglas de arquitectura',
)
await addFrontmatter(
  resolve(root, 'README.md'),
  join(references, 'sitio-documental.md'),
  'Sitio documental',
)
