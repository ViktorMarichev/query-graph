import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const npmCli = process.env.npm_execpath
const packCommand = npmCli ? process.execPath : 'npm'
const packArguments = npmCli
  ? [npmCli, 'pack', '--json', '--dry-run', '--ignore-scripts']
  : ['pack', '--json', '--dry-run', '--ignore-scripts']
const packOutput = execFileSync(packCommand, packArguments, {
  cwd: root,
  encoding: 'utf8',
  shell: !npmCli && process.platform === 'win32',
})
const [manifest] = JSON.parse(packOutput)
const files = new Set(manifest.files.map((file) => file.path))

for (const required of [
  'README.md',
  'definition.d.ts',
  'definition.js',
  'dsl.d.ts',
  'dsl.js',
  'index.d.ts',
  'index.js',
  'native.d.ts',
  'native.js',
  'package.json',
]) {
  assert(files.has(required), required + ' is missing from the package tarball')
}

for (const prefix of ['docs/', 'lib/', 'types/']) {
  assert(
    [...files].some((file) => file.startsWith(prefix)),
    prefix + ' is missing from the package tarball',
  )
}

execFileSync(
  process.execPath,
  [
    '-e',
    [
      "const dsl = require('@query-graph/core/dsl')",
      "if (typeof dsl.defineGraph !== 'function') throw new Error('DSL entrypoint is incomplete')",
      'if (Object.keys(require.cache).some((file) => /[\\\\/]native\\\\.js$/.test(file))) {',
      "  throw new Error('DSL entrypoint loaded the native adapter')",
      '}',
    ].join(';'),
  ],
  { cwd: root, stdio: 'inherit' },
)

execFileSync(
  process.execPath,
  [
    '-e',
    [
      "const core = require('@query-graph/core')",
      "if (typeof core.registerDefinition !== 'function') {",
      "  throw new Error('Package root did not load the platform binding')",
      '}',
    ].join(';'),
  ],
  { cwd: root, stdio: 'inherit' },
)

console.log('Package smoke checks passed for ' + manifest.id)
