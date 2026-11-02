import { readdir, readFile } from 'node:fs/promises'
import { dirname, extname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const dist = resolve(root, 'dist')

async function collectHtml(directory) {
  const files = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      files.push(...(await collectHtml(path)))
    } else if (entry.isFile() && entry.name.endsWith('.html')) {
      files.push(path)
    }
  }
  return files
}

function resolveTarget(pathname) {
  const clean = decodeURIComponent(pathname.split('#')[0].split('?')[0])
  if (!clean.startsWith('/')) return null
  if (clean.endsWith('/')) return join(dist, clean, 'index.html')
  const direct = join(dist, clean)
  if (extname(clean)) return direct
  return join(dist, `${clean}.html`)
}

const htmlFiles = await collectHtml(dist)
const missing = new Map()
const ignored = /^(\/\/|https?:|mailto:|tel:|data:|javascript:)/

for (const file of htmlFiles) {
  const html = await readFile(file, 'utf8')
  for (const match of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
    const value = match[1].trim()
    if (!value || value.startsWith('#') || ignored.test(value)) continue
    const target = resolveTarget(value)
    if (!target) continue
    const exists = await readFile(target).then(
      () => true,
      () => false,
    )
    if (!exists) {
      const page = relative(dist, file).split('\\').join('/')
      if (!missing.has(target)) missing.set(target, new Set())
      missing.get(target).add(page)
    }
  }
}

if (missing.size > 0) {
  console.error('Enlaces internos roto(s):')
  for (const [target, pages] of [...missing.entries()].sort()) {
    console.error(`- ${relative(dist, target)} (desde ${[...pages].sort().join(', ')})`)
  }
  process.exit(1)
}

console.log(`Enlaces internos: ${htmlFiles.length} páginas sin destinos rotos.`)
