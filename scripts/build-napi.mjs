import { spawnSync } from 'node:child_process'
import { appendFileSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import process from 'node:process'

const require = createRequire(import.meta.url)
const packagePath = require.resolve('@napi-rs/cli/package.json')
const packageDirectory = dirname(packagePath)
const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'))
const cliPath = resolve(packageDirectory, packageJson.bin.napi)
const environment = { ...process.env }

for (const name of Object.keys(environment)) {
  if (name.toLowerCase() === 'npm_new_version') delete environment[name]
}

const result = spawnSync(process.execPath, [cliPath, 'build', '--platform', ...process.argv.slice(2)], {
  env: environment,
  stdio: 'inherit',
})

if (result.error) throw result.error
if (result.signal) {
  throw new Error(`NAPI build terminated by signal ${result.signal}`)
}

if ((result.status ?? 1) === 0) {
  appendFileSync('index.js', "\nObject.assign(module.exports, require('./composition.js'))\n")
  appendFileSync(
    'index.d.ts',
    "\nexport { ComposedQueryGraph, CompiledQueryPlan, batchRelation, composeGraph } from './dsl.js'\n",
  )
}
process.exit(result.status ?? 1)
