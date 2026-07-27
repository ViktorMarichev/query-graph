import test from 'ava'

import { registerDefinition } from '../index'

const definition = {
  schemaVersion: 1,
  name: 'attributeValues',
  root: 'link',
  sources: [
    {
      key: 'link',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'idOwner', scalarType: 'int64' },
        { name: 'idControllerObjectValue', scalarType: 'int64' },
      ],
    },
    {
      key: 'value',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'value', scalarType: 'string', nullable: true },
      ],
    },
  ],
  parameters: [{ name: 'idOwner', scalarType: 'int64', required: true }],
  relations: [
    {
      name: 'value',
      from: 'link',
      to: 'value',
      required: true,
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idControllerObjectValue' },
        right: { kind: 'field', source: 'value', field: 'id' },
      },
    },
  ],
  constraints: [
    {
      name: 'owner',
      predicate: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idOwner' },
        right: { kind: 'parameter', name: 'idOwner' },
      },
    },
  ],
  projection: {
    fields: [
      {
        path: ['value', 'id'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'id' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'value'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'value' },
      },
    ],
  },
}

test('registers a definition as a native graph handle', (t) => {
  const graph = registerDefinition(definition)

  t.is(graph.name, 'attributeValues')
  t.is(graph.root, 'link')
  t.is(graph.sourceCount, 2)
  t.is(graph.relationCount, 1)
  t.true(graph.hasSource('value'))
  t.true(graph.hasField('value', 'value'))
  t.true(graph.hasParameter('idOwner'))
  t.true(graph.hasRelation('value'))
  t.deepEqual(graph.selectableFields(), ['value.id', 'value.value'])
})

test('returns definition validation errors to Node.js', (t) => {
  const error = t.throws(() =>
    registerDefinition({
      ...definition,
      root: 'missing',
    }),
  )

  t.regex(error.message, /UnknownRoot/)
  t.regex(error.message, /root source "missing" is not defined/)
})
