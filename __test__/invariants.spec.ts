import test from 'ava'

import { constraint, defineGraph, eq, param, project, requiredParameter, source } from '../definition.js'
import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

test('accepts safe JavaScript integers for int64 parameters', (t) => {
  const root = source('root', { id: 'int64' })
  const id = requiredParameter('id', 'int64')
  const graph = registerDefinition(
    defineGraph({
      name: 'int64Parameters',
      root,
      sources: [root],
      parameters: [id],
      constraints: [constraint('id', eq(root.field('id'), param(id)))],
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
      constraints: [constraint('amount', eq(root.field('amount'), param(amount)))],
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
