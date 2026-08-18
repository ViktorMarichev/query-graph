import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  asc,
  batchQuery,
  batchRelation,
  composeGraph,
  constraint,
  defineGraph,
  eq,
  nullable,
  ordering,
  param,
  project,
  requiredListParameter,
  requiredParameter,
  source,
} from '../dsl.js'
import type { QueryGraphError, ResultOf } from '../dsl.js'

const items = source('items', {
  id: 'int64',
  ownerId: 'int64',
  title: nullable('string'),
})
const ownerId = requiredParameter('ownerId', 'int64')
const definition = defineGraph({
  name: 'items',
  root: items,
  sources: [items],
  parameters: [ownerId],
  constraints: [
    constraint({
      predicate: eq(items.field('ownerId'), param(ownerId)),
    }),
  ],
  projection: [
    project({ path: 'id', expression: items.field('id'), default: true }),
    project({ path: 'title', expression: items.field('title') }),
  ],
  orderings: [
    ordering({
      name: 'default',
      by: [asc(items.field('id'))],
      default: true,
    }),
  ],
})
const graph = registerDefinition(definition).withRelationalMapping({
  sources: {
    items: {
      table: { schema: 'dbo', name: 'Items' },
      columns: { ownerId: 'owner_id' },
    },
  },
})

test('preserves the wire definition, diagnostics, SQL, bindings, and projection metadata', (t) => {
  t.deepEqual(definition, {
    schemaVersion: 10,
    name: 'items',
    root: 'items',
    sources: [
      {
        key: 'items',
        fields: [
          { name: 'id', scalarType: 'int64' },
          { name: 'ownerId', scalarType: 'int64' },
          { name: 'title', scalarType: 'string', nullable: true },
        ],
      },
    ],
    parameters: [{ name: 'ownerId', scalarType: 'int64', required: true }],
    relations: [],
    constraints: [
      {
        predicate: {
          kind: 'eq',
          left: { kind: 'field', source: 'items', field: 'ownerId' },
          right: { kind: 'parameter', name: 'ownerId' },
        },
      },
    ],
    projection: {
      fields: [
        {
          path: ['id'],
          expression: { kind: 'field', source: 'items', field: 'id' },
          selectedByDefault: true,
        },
        {
          path: ['title'],
          expression: { kind: 'field', source: 'items', field: 'title' },
        },
      ],
      objects: [],
    },
    orderings: [
      {
        name: 'default',
        orderBy: [
          {
            expression: { kind: 'field', source: 'items', field: 'id' },
            direction: 'asc',
          },
        ],
        default: true,
      },
    ],
  })

  const operation = {
    select: ['id', 'title'],
    parameters: { ownerId: 7 },
  } as const
  type Row = ResultOf<typeof definition, typeof operation>
  const typedRow: Row = { id: 1, title: null }
  t.deepEqual(typedRow, { id: 1, title: null })

  const sqlServer = graph.compileSqlServer(operation)
  t.is(
    sqlServer.sql,
    [
      'SELECT',
      '  [t0].[id] AS [c0],',
      '  [t0].[title] AS [c1]',
      'FROM [dbo].[Items] AS [t0]',
      'WHERE',
      '  ([t0].[owner_id] = @p0)',
      'ORDER BY',
      '  [t0].[id] ASC',
    ].join('\n'),
  )
  t.deepEqual(sqlServer.bindings, [{ name: 'p0', parameter: 'ownerId', scalarType: 'int64' }])
  t.deepEqual(sqlServer.columns, [
    { name: 'c0', path: 'id', scalarType: 'int64', nullable: false, relations: [] },
    { name: 'c1', path: 'title', scalarType: 'string', nullable: true, relations: [] },
  ])

  const oracle = graph.compileOracle(operation)
  t.is(
    oracle.sql,
    [
      'SELECT',
      '  "t0"."id" AS "c0",',
      '  "t0"."title" AS "c1"',
      'FROM "dbo"."Items" "t0"',
      'WHERE',
      '  ("t0"."owner_id" = :p0)',
      'ORDER BY',
      '  "t0"."id" ASC',
    ].join('\n'),
  )
  t.deepEqual(oracle.bindings, sqlServer.bindings)
  t.deepEqual(oracle.columns, sqlServer.columns)

  const error = t.throws(() =>
    registerDefinition({
      ...definition,
      root: 'missing',
    }),
  ) as QueryGraphError
  t.deepEqual(error.issues, [
    {
      code: 'unknownRoot',
      location: 'root',
      message: 'root source "missing" is not defined',
    },
  ])
})

test('preserves lazy batch plan metadata and deferred list bindings', (t) => {
  const itemDetails = source('itemDetails', {
    itemId: 'int64',
    value: 'string',
  })
  const itemIds = requiredListParameter('itemIds', 'int64')
  const childDefinition = defineGraph({
    name: 'itemDetails',
    root: itemDetails,
    sources: [itemDetails],
    parameters: [itemIds],
    constraints: [
      constraint({
        predicate: {
          kind: 'inParameter',
          expression: itemDetails.field('itemId'),
          parameter: itemIds.name,
        },
      }),
    ],
    projection: [
      project({ path: 'itemId', expression: itemDetails.field('itemId') }),
      project({ path: 'value', expression: itemDetails.field('value') }),
    ],
  })
  const childGraph = registerDefinition(childDefinition).withRelationalMapping({
    sources: { itemDetails: { table: 'ItemDetails' } },
  })
  const query = batchQuery({
    graph: childGraph,
    key: { path: 'itemId', parameter: itemIds },
  })
  const details = batchRelation({
    name: 'details',
    from: 'id',
    query,
    cardinality: 'many',
  })
  const composed = composeGraph({ root: graph, relations: [details] })
  const plan = composed.compileSqlServerPlan({
    select: ['title', 'details.value'],
    parameters: { ownerId: 7 },
  })

  t.deepEqual(plan.batches, [
    {
      name: 'details',
      parentKey: 'id',
      childKey: 'itemId',
      keyParameter: 'itemIds',
      parameters: {},
      cardinality: 'many',
      parentKeyInjected: true,
      childKeyInjected: true,
    },
  ])
  t.deepEqual(plan.compileBatch('details', [3, 5]).bindings, [
    { name: 'p0', parameter: 'itemIds', scalarType: 'int64', index: 0 },
    { name: 'p1', parameter: 'itemIds', scalarType: 'int64', index: 1 },
  ])
})
