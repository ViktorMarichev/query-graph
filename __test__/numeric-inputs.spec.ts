import test from 'ava'

import {
  and,
  asc,
  coalesce,
  constraint,
  defineGraph,
  eq,
  firstBy,
  inParameter,
  integer,
  ordering,
  project,
  relation,
  requiredListParameter,
  requiredParameter,
  source,
} from '../definition.js'
import type { GraphDefinitionInput, QueryGraphError, QueryOperation } from '../definition.js'
import { composeGraph, registerDefinition } from '../index.js'

const item = source('item', { id: 'int64', amount: 'decimal' })
const mapping = { sources: { item: { table: 'Item' } } }
const safeIntegers = [
  0,
  42,
  -42,
  -(2 ** 31),
  2 ** 32 - 1,
  2 ** 32,
  -(2 ** 31) - 1,
  5_000_000_000,
  -5_000_000_000,
  Number.MAX_SAFE_INTEGER,
  -Number.MAX_SAFE_INTEGER,
]
const invalidIntegers = [1.5, Number.MAX_SAFE_INTEGER + 1, -Number.MAX_SAFE_INTEGER - 1, '5000000000']

function integerDefinition(value: number, authoring: 'wire' | 'dsl'): GraphDefinitionInput {
  if (authoring === 'dsl') {
    return defineGraph({
      name: 'integerInputs',
      root: item,
      sources: [item],
      projection: [project('value', integer(value), { default: true })],
    })
  }
  return {
    schemaVersion: 12,
    name: 'integerInputs',
    root: 'item',
    sources: [{ key: 'item', fields: [{ name: 'id', scalarType: 'int64' }] }],
    projection: {
      fields: [
        {
          path: ['value'],
          expression: { kind: 'literal', value: { kind: 'integer', value } },
          selectedByDefault: true,
        },
      ],
    },
  }
}

for (const authoring of ['wire', 'dsl'] as const) {
  test(`preserves safe integer literals exactly through ${authoring} definitions`, (t) => {
    for (const value of safeIntegers) {
      const graph = registerDefinition(integerDefinition(value, authoring)).withRelationalMapping(mapping)
      const sqlServer = graph.compileSqlServer({})
      const oracle = graph.compileOracle({})
      t.is(sqlServer.sql, `SELECT\n  ${value} AS [c0]\nFROM [Item] AS [t0]`)
      t.is(oracle.sql, `SELECT\n  ${value} AS "c0"\nFROM "Item" "t0"`)
      for (const statement of [sqlServer, oracle]) {
        t.deepEqual(statement.bindings, [])
        t.deepEqual(statement.columns, [
          { name: 'c0', path: 'value', scalarType: 'int64', nullable: false, relations: [] },
        ])
      }
    }
  })
}

test('decodes integer literals nested in functions, relation predicates, and firstBy ordering', (t) => {
  const detail = source('detail', { id: 'int64', rank: 'int64' })
  const graph = registerDefinition(
    defineGraph({
      name: 'nestedIntegers',
      root: item,
      sources: [item, detail],
      relations: [
        relation(
          'detail',
          item,
          detail,
          and(eq(item.field('id'), detail.field('id')), eq(detail.field('rank'), integer(5_000_000_000))),
          { selection: firstBy(asc(coalesce(detail.field('rank'), integer(-5_000_000_000)))) },
        ),
      ],
      constraints: [constraint(eq(item.field('id'), integer(-Number.MAX_SAFE_INTEGER)))],
      projection: [
        project('rank', coalesce(detail.field('rank'), integer(Number.MAX_SAFE_INTEGER)), { default: true }),
      ],
    }),
  ).withRelationalMapping({ sources: { item: { table: 'Item' }, detail: { table: 'Detail' } } })

  const sqlServer = graph.compileSqlServer({})
  const oracle = graph.compileOracle({})
  t.true(sqlServer.sql.includes('COALESCE([t1].[rank], 9007199254740991) AS [c0]'))
  t.true(oracle.sql.includes('COALESCE("t1"."rank", 9007199254740991) AS "c0"'))
  t.true(sqlServer.sql.includes('ORDER BY COALESCE([t1].[rank], -5000000000) ASC'))
  t.true(oracle.sql.includes('ORDER BY COALESCE("t1"."rank", -5000000000) ASC'))
  for (const statement of [sqlServer, oracle]) {
    t.true(statement.sql.includes(' = 5000000000)'))
    t.true(statement.sql.includes(' = -9007199254740991)'))
    t.deepEqual(statement.bindings, [])
  }
})

const paginated = registerDefinition(
  defineGraph({
    name: 'paginationInputs',
    root: item,
    sources: [item],
    projection: [project('id', item.field('id'), { default: true })],
    orderings: [ordering({ name: 'id', by: [asc(item.field('id'))], default: true })],
  }),
).withRelationalMapping(mapping)
const composed = composeGraph({ root: paginated, relations: [] })
type Pagination = Pick<QueryOperation, 'offset' | 'limit'>
const paginationCompilers = [
  (operation: Pagination) => paginated.compileSqlServer(operation),
  (operation: Pagination) => paginated.compileOracle(operation),
  (operation: Pagination) => composed.compileSqlServerPlan(operation).root,
  (operation: Pagination) => composed.compileOraclePlan(operation).root,
]

test('preserves safe integer pagination in ordinary and composed plans', (t) => {
  for (const value of [1, 2 ** 32 - 1, 2 ** 32, 5_000_000_000, Number.MAX_SAFE_INTEGER]) {
    for (const operation of [{ offset: value, limit: 7 }, { offset: 3, limit: value }, { limit: value }]) {
      const statements = paginationCompilers.map((compile) => compile(operation))
      for (const statement of statements) {
        t.true(statement.sql.endsWith(`\nOFFSET ${operation.offset ?? 0} ROWS FETCH NEXT ${operation.limit} ROWS ONLY`))
        t.deepEqual(statement.bindings, [])
      }
      t.deepEqual(statements[2], statements[0])
      t.deepEqual(statements[3], statements[1])
    }
  }
})

test('preserves rejection of fractional, unsafe, and string integer literals', (t) => {
  for (const value of invalidIntegers) {
    // The cast deliberately exercises untyped JavaScript callers at both boundaries.
    t.throws(() => integer(value as number), {
      instanceOf: TypeError,
      message: 'Integer literal must be a safe JavaScript integer',
    })
    const error = t.throws(() => registerDefinition(integerDefinition(value as number, 'wire'))) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_DEFINITION_WIRE_INVALID')
    t.is(error.phase, 'definition')
    t.deepEqual(
      error.issues.map(({ code, location }) => ({ code, location })),
      [{ code: 'invalidWireFormat', location: 'definition' }],
    )
    if (value === 1.5) {
      t.is(error.message, 'Invalid query graph definition: invalid type: floating point `1.5`, expected i64')
    }
  }
})

test('preserves pagination wire errors and zero-limit validation', (t) => {
  for (const compile of paginationCompilers) {
    for (const value of [-1, ...invalidIntegers]) {
      for (const property of ['offset', 'limit']) {
        const error = t.throws(() => compile({ [property]: value } as Pagination)) as QueryGraphError
        t.is(error.code, 'QUERY_GRAPH_OPERATION_WIRE_INVALID')
        t.is(error.phase, 'operation')
        t.deepEqual(
          error.issues.map(({ code, location }) => ({ code, location })),
          [{ code: 'invalidWireFormat', location: 'operation' }],
        )
        if (value === 1.5) {
          t.is(error.message, 'Invalid query operation: invalid type: floating point `1.5`, expected u64')
        }
      }
    }
    const error = t.throws(() => compile({ limit: 0 })) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_OPERATION_INVALID')
    t.is(error.phase, 'operation')
    t.deepEqual(
      error.issues.map(({ code, location }) => ({ code, location })),
      [{ code: 'invalidPagination', location: 'limit' }],
    )
  }
})

test('keeps literal-shaped JSON runtime parameters as parameter data', (t) => {
  const payload = requiredParameter('payload', 'json')
  const graph = registerDefinition(
    defineGraph({
      name: 'jsonNumberInput',
      root: item,
      sources: [item],
      parameters: [payload],
      projection: [project('payload', payload, { default: true })],
    }),
  ).withRelationalMapping(mapping)
  const parameters = { payload: { kind: 'literal', value: { kind: 'integer', value: 5_000_000_000 } } }
  for (const statement of [graph.compileSqlServer({ parameters }), graph.compileOracle({ parameters })]) {
    t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'payload', scalarType: 'json' }])
  }
  t.deepEqual(parameters.payload, { kind: 'literal', value: { kind: 'integer', value: 5_000_000_000 } })
})

test('preserves scalar int64, list int64, and decimal runtime bindings', (t) => {
  const id = requiredParameter('id', 'int64')
  const ids = requiredListParameter('ids', 'int64')
  const amount = requiredParameter('amount', 'decimal')
  const graph = registerDefinition(
    defineGraph({
      name: 'numericParameters',
      root: item,
      sources: [item],
      parameters: [id, ids, amount],
      constraints: [
        constraint(eq(item.field('id'), id)),
        constraint(inParameter(item.field('id'), ids)),
        constraint(eq(item.field('amount'), amount)),
      ],
      projection: [project('id', item.field('id'), { default: true })],
    }),
  ).withRelationalMapping(mapping)

  for (const parameters of [
    { id: 5_000_000_000, ids: [5_000_000_000, -5_000_000_000], amount: 123.125 },
    { id: Number.MAX_SAFE_INTEGER, ids: [Number.MAX_SAFE_INTEGER, -Number.MAX_SAFE_INTEGER], amount: '1234567890.125' },
    { id: '9007199254740992', ids: ['9223372036854775807', '-9223372036854775808'], amount: 5_000_000_000 },
  ]) {
    const sqlServer = graph.compileSqlServer({ parameters })
    const oracle = graph.compileOracle({ parameters })
    for (const statement of [sqlServer, oracle]) {
      t.deepEqual(statement.bindings, [
        { name: 'p0', parameter: 'id', scalarType: 'int64' },
        { name: 'p1', parameter: 'ids', scalarType: 'int64', index: 0 },
        { name: 'p2', parameter: 'ids', scalarType: 'int64', index: 1 },
        { name: 'p3', parameter: 'amount', scalarType: 'decimal' },
      ])
    }
    t.true(sqlServer.sql.includes('([t0].[id] IN (@p1, @p2))'))
    t.true(oracle.sql.includes('("t0"."id" IN (:p1, :p2))'))
  }
})
