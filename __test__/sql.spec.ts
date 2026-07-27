import test from 'ava'

import { registerDefinition } from '../index.js'

const definition = {
  schemaVersion: 1,
  name: 'attributeValues',
  root: 'link',
  sources: [
    {
      key: 'link',
      fields: [
        { name: 'idOwner', scalarType: 'int64' },
        { name: 'idValue', scalarType: 'int64' },
        { name: 'order', scalarType: 'int32' },
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
        left: { kind: 'field', source: 'link', field: 'idValue' },
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
        selectedByDefault: true,
      },
    ],
  },
  defaultOrderBy: [
    {
      expression: { kind: 'field', source: 'link', field: 'order' },
      direction: 'asc',
    },
  ],
}

const mapping = {
  sources: {
    link: {
      table: { schema: 'dbo', name: 'ControllerAttributeValueLink' },
      columns: { idOwner: 'owner_id' },
    },
    value: {
      table: 'ControllerObjectValue',
    },
  },
}

test('compiles a mapped graph to SQL Server', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)

  const statement = graph.compileSqlServer({
    parameters: { idOwner: 42 },
    limit: 20,
  })
  t.is(graph.name, 'attributeValues')
  t.deepEqual(statement.fields, ['value.id', 'value.value'])
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idOwner' }])
  t.regex(statement.sql, /FROM \[dbo\]\.\[ControllerAttributeValueLink\] AS \[link\]/)
  t.regex(statement.sql, /INNER JOIN \[ControllerObjectValue\] AS \[value\]/)
  t.regex(statement.sql, /\(\[link\]\.\[owner_id\] = @p0\)/)
  t.regex(statement.sql, /OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY$/)
})

test('returns relational mapping errors to Node.js', (t) => {
  const graph = registerDefinition(definition)
  const error = t.throws(() =>
    graph.withRelationalMapping({
      sources: {
        missing: { table: 'Missing' },
      },
    }),
  )

  t.regex(error.message, /UnknownSource/)
  t.regex(error.message, /MissingSource/)
})
