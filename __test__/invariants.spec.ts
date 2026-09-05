import test from 'ava'
// @ts-expect-error The DSL type-test project does not include Node runtime declarations.
import { spawnSync } from 'node:child_process'
// @ts-expect-error The DSL type-test project does not include Node runtime declarations.
import { execPath } from 'node:process'
// @ts-expect-error The DSL type-test project does not include Node runtime declarations.
import { fileURLToPath } from 'node:url'

import { constraint, defineGraph, eq, param, project, requiredParameter, source } from '../definition.js'
import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

test('returns definition diagnostics for backward exists correlation without aborting Node', (t) => {
  const script = `
    const { registerDefinition } = require('./index.js')
    const { constraint, defineGraph, eq, exists, relation, source } = require('./dsl.js')
    const root = source('root', { id: 'int64' })
    const middle = source('middle', { id: 'int64' })
    const leaf = source('leaf', { id: 'int64' })
    const other = source('other', { id: 'int64' })
    const relations = [[root, middle], [middle, leaf], [root, other]].map(([from, to]) =>
      relation({ name: to.key, from, to, on: eq(from.field('id'), to.field('id')) }),
    )
    const errors = [middle, root, other].map((target) => {
      try {
        registerDefinition(defineGraph({
          name: 'backwardExists', root, sources: [root, middle, leaf, other], relations,
          constraints: [constraint({ predicate: exists(target, undefined, { from: leaf }) })],
        }))
        return null
      } catch (error) {
        return {
          name: error.name, code: error.code, phase: error.phase,
          issues: error.issues.map(({ code, location }) => ({ code, location })),
        }
      }
    })
    process.stdout.write(JSON.stringify(errors))
  `
  // Keep native panic regressions in a child so they cannot abort the AVA worker.
  const result = spawnSync(execPath, ['-e', script], {
    cwd: fileURLToPath(new URL('..', import.meta.url)),
    encoding: 'utf8',
    timeout: 10_000,
  })

  t.is(result.error, undefined)
  t.is(result.status, 0, result.stderr)
  const fromIssue = { code: 'invalidExistsSource', location: 'constraints[0].predicate.from' }
  const expectedError = {
    name: 'QueryGraphError',
    code: 'QUERY_GRAPH_DEFINITION_INVALID',
    phase: 'definition',
    issues: [fromIssue],
  }
  t.deepEqual(JSON.parse(result.stdout), [
    expectedError,
    {
      ...expectedError,
      issues: [fromIssue, { code: 'invalidExistsSource', location: 'constraints[0].predicate.source' }],
    },
    expectedError,
  ])
})

test('accepts safe JavaScript integers for int64 parameters', (t) => {
  const root = source('root', { id: 'int64' })
  const id = requiredParameter('id', 'int64')
  const graph = registerDefinition(
    defineGraph({
      name: 'int64Parameters',
      root,
      sources: [root],
      parameters: [id],
      constraints: [constraint(eq(root.field('id'), param(id)))],
      projection: [project('id', root.field('id'), { default: true })],
    }),
  ).withRelationalMapping({
    sources: {
      root: { table: 'Root' },
    },
  })

  const statement = graph.compileSqlServer({
    parameters: {
      id: 5_000_000_000,
    },
  })

  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'id', scalarType: 'int64' }])
})

test('rejects non-decimal strings for decimal parameters', (t) => {
  const root = source('root', { amount: 'decimal' })
  const amount = requiredParameter('amount', 'decimal')
  const graph = registerDefinition(
    defineGraph({
      name: 'decimalParameters',
      root,
      sources: [root],
      parameters: [amount],
      constraints: [constraint(eq(root.field('amount'), param(amount)))],
      projection: [project('amount', root.field('amount'), { default: true })],
    }),
  ).withRelationalMapping({
    sources: {
      root: { table: 'Root' },
    },
  })

  const error = t.throws(() =>
    graph.compileSqlServer({
      parameters: {
        amount: 'NaN',
      },
    }),
  ) as QueryGraphError

  t.is(error.code, 'QUERY_GRAPH_OPERATION_INVALID')
  t.like(error.issues[0], {
    code: 'invalidParameterType',
    location: 'parameters.amount',
  })
})

test('rejects unknown fields at every Node wire boundary', (t) => {
  const root = source('root', { id: 'int64' })
  const definition = defineGraph({
    name: 'strictWire',
    root,
    sources: [root],
    projection: [project('id', root.field('id'), { default: true })],
  })

  const definitionError = t.throws(() =>
    registerDefinition({
      ...definition,
      // @ts-expect-error This intentionally exercises the runtime wire boundary.
      unexpected: true,
    }),
  ) as QueryGraphError
  t.is(definitionError.code, 'QUERY_GRAPH_DEFINITION_WIRE_INVALID')
  t.is(definitionError.phase, 'definition')
  t.like(definitionError.issues[0], { code: 'invalidWireFormat' })
  t.regex(definitionError.message, /unexpected/)

  const graph = registerDefinition(definition)
  const mappingError = t.throws(() =>
    graph.withRelationalMapping({
      sources: {
        root: {
          table: 'Root',
          // @ts-expect-error This intentionally exercises the runtime wire boundary.
          colums: {},
        },
      },
    }),
  ) as QueryGraphError
  t.is(mappingError.code, 'QUERY_GRAPH_MAPPING_WIRE_INVALID')
  t.is(mappingError.phase, 'mapping')
  t.like(mappingError.issues[0], { code: 'invalidWireFormat' })
  t.regex(mappingError.message, /colums/)

  const mapped = graph.withRelationalMapping({
    sources: {
      root: { table: 'Root' },
    },
  })
  const operationError = t.throws(() =>
    mapped.compileSqlServer({
      // @ts-expect-error This intentionally exercises the runtime wire boundary.
      limt: 1,
    }),
  ) as QueryGraphError
  t.is(operationError.code, 'QUERY_GRAPH_OPERATION_WIRE_INVALID')
  t.is(operationError.phase, 'operation')
  t.like(operationError.issues[0], { code: 'invalidWireFormat' })
  t.regex(operationError.message, /limt/)
})
