// Builds the golden fixtures from the real Electron parser and writer.
// Run from this folder: bun generate.ts /path/to/md-kanban
// It copies every card from the Electron repo's tasks/ and examples/ into cards/,
// then writes <name>.json (parseTaskFile) and <name>.written (stringifyTaskFile)
// next to every card in cards/ and edge/. edge/ holds hand-written odd cases.
import { cpSync, readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import path from 'node:path'

const repo = process.argv[2]
if (!repo) throw new Error('usage: bun generate.ts /path/to/md-kanban')
const { parseTaskFile, stringifyTaskFile } = await import(path.join(repo, 'src/lib/markdown.ts'))

function walk(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name)
    return e.isDirectory() ? walk(p) : e.name.endsWith('.md') ? [p] : []
  })
}

mkdirSync('cards', { recursive: true })
for (const top of ['tasks', 'examples']) {
  for (const file of walk(path.join(repo, top))) {
    const flat = path.relative(repo, file).split(path.sep).join('__')
    cpSync(file, path.join('cards', flat))
  }
}

const NOW = '2026-01-02T03:04:05.678Z'
const RealDate = Date
// Freeze the clock so missing created/modified values are stable.
globalThis.Date = class extends RealDate {
  constructor(...args: any[]) {
    if (args.length) super(...(args as [any]))
    else super(NOW)
  }
  static now() {
    return new RealDate(NOW).getTime()
  }
} as DateConstructor

let count = 0
for (const dir of ['cards', 'edge']) {
  for (const file of walk(dir)) {
    const raw = readFileSync(file, 'utf-8')
    const fileName = path.basename(file)
    const task = parseTaskFile(raw, { project: 'p', workspace: '', relPath: fileName, fileName })
    const { project, workspace, relPath, fileName: _f, ...fields } = task
    writeFileSync(file.replace(/\.md$/, '.json'), JSON.stringify(fields, null, 2) + '\n')
    writeFileSync(file.replace(/\.md$/, '.written'), stringifyTaskFile(task))
    count++
  }
}
console.log(`${count} fixtures written`)
