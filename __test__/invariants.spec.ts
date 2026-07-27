import test from 'ava'

import { registerDefinition } from '../index.js'

test('accepts safe JavaScript integers for int64 parameters', (t) => {
  const graph = registerDefinition({
    schemaVersion: 1,
    name: 'int64Parameters',
    root: 'root',
    sources: [
      {
        key: 'root',
        fields: [{ name: 'id', scalarType: 'int64' }],
      },
    ],
    parameters: [{ name: 'id', scalarType: 'int64', required: true }],
    constraints: [
      {
        name: 'id',
        predicate: {
          kind: 'eq',
          left: { kind: 'field', source: 'root', field: 'id' },
          right: { kind: 'parameter', name: 'id' },
        },
      },
    ],
    projection: {
      fields: [
        {
          path: ['id'],
          expression: { kind: 'field', source: 'root', field: 'id' },
          selectedByDefault: true,
        },
      ],
    },
  }).withRelationalMapping({
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
