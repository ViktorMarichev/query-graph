import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const rootDirectory = join(dirname(fileURLToPath(import.meta.url)), '..')
const checkOnly = process.argv.includes('--check')
const corePackageName = '@query-graph/core'
const semverPattern =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/

const paths = {
  rootPackage: join(rootDirectory, 'package.json'),
  dslPackage: join(rootDirectory, 'packages', 'dsl-object', 'package.json'),
  yarnLock: join(rootDirectory, 'yarn.lock'),
  generatedBinding: join(rootDirectory, 'native.js'),
  rootCargo: join(rootDirectory, 'Cargo.toml'),
  coreCargo: join(rootDirectory, 'crates', 'query-graph-core', 'Cargo.toml'),
  cargoLock: join(rootDirectory, 'Cargo.lock'),
}

const read = (path) => readFileSync(path, 'utf8')
const write = (path, value) => writeFileSync(path, value)
const readJson = (path) => JSON.parse(read(path))
const writeJson = (path, value) => write(path, `${JSON.stringify(value, null, 2)}\n`)

const rootPackage = readJson(paths.rootPackage)
const version = rootPackage.version

if (rootPackage.name !== corePackageName) {
  throw new Error(`Expected root package name ${corePackageName}, received ${rootPackage.name}`)
}

if (!semverPattern.test(version)) {
  throw new Error(`Root package version is not valid SemVer: ${version}`)
}

const replaceOnce = (content, pattern, replacement, label) => {
  let matches = 0
  const result = content.replace(pattern, (...args) => {
    matches += 1
    return typeof replacement === 'function' ? replacement(...args) : replacement
  })

  if (matches !== 1) {
    throw new Error(`Expected exactly one ${label}, found ${matches}`)
  }

  return result
}

const readCargoPackageVersion = (content) => {
  const packageSection = content.match(/\[package\]([\s\S]*?)(?=\n\[|$)/)
  return packageSection?.[1].match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1]
}

const readCoreDependencyVersion = (content) =>
  content.match(/^query-graph-core\s*=\s*\{[^}\n]*\bversion\s*=\s*"([^"]+)"/m)?.[1]

const readYarnDslPeerRange = (content) =>
  content.match(/peerDependencies:\r?\n\s+"@query-graph\/core":\s+"?([^"\r\n]+)"?/m)?.[1]

const readGeneratedBindingVersion = (content) => {
  const versions = new Set([...content.matchAll(/bindingPackageVersion !== '([^']+)'/g)].map((match) => match[1]))

  if (versions.size === 0) return undefined
  if (versions.size === 1) return versions.values().next().value
  return [...versions].join(', ')
}

const readCargoLockVersion = (content, packageName) => {
  const section = content
    .split('[[package]]')
    .find((candidate) => candidate.match(/^\s*name\s*=\s*"([^"]+)"/m)?.[1] === packageName)

  return section?.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1]
}

const collectMismatches = ({ includeGeneratedBinding = true } = {}) => {
  const dslPackage = readJson(paths.dslPackage)
  const rootCargo = read(paths.rootCargo)
  const yarnLock = read(paths.yarnLock)
  const generatedBinding = includeGeneratedBinding ? read(paths.generatedBinding) : undefined
  const coreCargo = read(paths.coreCargo)
  const cargoLock = read(paths.cargoLock)
  const expectedPeerRange = `^${version}`
  const actual = [
    ['DSL package version', dslPackage.version],
    ['DSL core peer dependency', dslPackage.peerDependencies?.[corePackageName]],
    ['DSL core workspace dependency', dslPackage.devDependencies?.[corePackageName]],
    ['Yarn DSL core peer dependency', readYarnDslPeerRange(yarnLock)],
    ['root Cargo package version', readCargoPackageVersion(rootCargo)],
    ['core Cargo package version', readCargoPackageVersion(coreCargo)],
    ['root Cargo core dependency version', readCoreDependencyVersion(rootCargo)],
    ['Cargo.lock Node adapter version', readCargoLockVersion(cargoLock, 'query_graph')],
    ['Cargo.lock core version', readCargoLockVersion(cargoLock, 'query-graph-core')],
  ]
  const expected = [
    version,
    expectedPeerRange,
    'workspace:^',
    expectedPeerRange,
    version,
    version,
    version,
    version,
    version,
  ]
  if (includeGeneratedBinding) {
    actual.push(['generated binding version', readGeneratedBindingVersion(generatedBinding)])
    expected.push(version)
  }

  return actual.flatMap(([label, actualValue], index) =>
    actualValue === expected[index]
      ? []
      : [`${label}: expected ${expected[index]}, received ${actualValue ?? '<missing>'}`],
  )
}

const assertSynchronized = (options) => {
  const mismatches = collectMismatches(options)

  if (mismatches.length > 0) {
    throw new Error(`Version metadata is not synchronized:\n- ${mismatches.join('\n- ')}`)
  }
}

if (checkOnly) {
  assertSynchronized()
  console.log(`Version metadata is synchronized at ${version}`)
  process.exit(0)
}

const dslPackage = readJson(paths.dslPackage)
dslPackage.version = version
dslPackage.peerDependencies = {
  ...dslPackage.peerDependencies,
  [corePackageName]: `^${version}`,
}
dslPackage.devDependencies = {
  ...dslPackage.devDependencies,
  [corePackageName]: 'workspace:^',
}
writeJson(paths.dslPackage, dslPackage)

const yarnLock = replaceOnce(
  read(paths.yarnLock),
  /(peerDependencies:\r?\n\s+"@query-graph\/core":\s*)[^\r\n]+/m,
  (_, prefix) => `${prefix}^${version}`,
  'Yarn DSL core peer dependency',
)
write(paths.yarnLock, yarnLock)

const rootCargo = replaceOnce(
  read(paths.rootCargo),
  /(\[package\][\s\S]*?^\s*version\s*=\s*")[^"]+(")/m,
  (_, prefix, suffix) => `${prefix}${version}${suffix}`,
  'root Cargo package version',
)
const synchronizedRootCargo = replaceOnce(
  rootCargo,
  /(^query-graph-core\s*=\s*\{[^}\n]*\bversion\s*=\s*")[^"]+(")/m,
  (_, prefix, suffix) => `${prefix}${version}${suffix}`,
  'root Cargo core dependency version',
)
write(paths.rootCargo, synchronizedRootCargo)

const coreCargo = replaceOnce(
  read(paths.coreCargo),
  /(\[package\][\s\S]*?^\s*version\s*=\s*")[^"]+(")/m,
  (_, prefix, suffix) => `${prefix}${version}${suffix}`,
  'core Cargo package version',
)
write(paths.coreCargo, coreCargo)

try {
  execFileSync('cargo', ['metadata', '--format-version', '1'], {
    cwd: rootDirectory,
    stdio: ['ignore', 'ignore', 'inherit'],
  })
} catch {
  throw new Error('Cargo failed to refresh Cargo.lock after synchronizing versions')
}

assertSynchronized({ includeGeneratedBinding: false })
console.log(`Synchronized npm and Cargo version metadata at ${version}`)
