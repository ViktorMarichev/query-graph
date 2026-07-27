import test from 'ava'

import { constraint, defineGraph, eq, param, project, requiredParameter, source } from '../definition.js'
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

  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'id' }])
})
