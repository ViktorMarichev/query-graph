import test from 'ava'

import { concat, defineGraph, literal, nullable, param, project, requiredParameter, source } from '../definition.js'
import type { Expression, GraphDefinitionInput, ParameterDefinition, QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

const item = source('item', { name: nullable('string') })
const columns = (isNullable: boolean) => [
  { name: 'c0', path: 'value', scalarType: 'string', nullable: isNullable, relations: [] },
]
const sqlServerQuery = (expression: string) => `SELECT\n  ${expression} AS [c0]\nFROM [Item] AS [t0]`
const oracleQuery = (expression: string) => `SELECT\n  ${expression} AS "c0"\nFROM "Item" "t0"`

function concatGraph(args: Expression[], authoring: 'wire' | 'dsl', parameters: ParameterDefinition[] = []) {
  const definition: GraphDefinitionInput =
    authoring === 'dsl'
      ? defineGraph({
          name: 'concatInputs',
          root: item,
          sources: [item],
          parameters,
          projection: [project('value', concat(args[0]!, ...args.slice(1)), { default: true })],
        })
      : {
          schemaVersion: 11,
          name: 'concatInputs',
          root: 'item',
          sources: [{ key: 'item', fields: [{ name: 'name', scalarType: 'string', nullable: true }] }],
          parameters,
          projection: {
            fields: [
              {
                path: ['value'],
                expression: { kind: 'function', name: 'concat', arguments: args },
                selectedByDefault: true,
              },
            ],
          },
        }
  return registerDefinition(definition).withRelationalMapping({ sources: { item: { table: 'Item' } } })
}

test('rejects an empty wire concat before SQL rendering', (t) => {
  const error = t.throws(() => concatGraph([], 'wire')) as QueryGraphError
  t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
  t.is(error.phase, 'definition')
  t.deepEqual(
    error.issues.map(({ code, location }) => ({ code, location })),
    [{ code: 'invalidFunctionArity', location: 'projection.fields[0].expression' }],
  )
})

for (const authoring of ['wire', 'dsl'] as const) {
  test(`renders unary concat with two SQL Server arguments through ${authoring}`, (t) => {
    for (const fixture of [
      { expression: literal(null), sqlServer: 'NULL', oracle: 'NULL', nullable: true },
      { expression: item.field('name'), sqlServer: '[t0].[name]', oracle: '"t0"."name"', nullable: true },
      { expression: literal('hello'), sqlServer: "N'hello'", oracle: "'hello'", nullable: false },
    ]) {
      const graph = concatGraph([fixture.expression], authoring)
      for (const version of ['2012', '2022'] as const) {
        const statement = graph.compileSqlServer({}, { version })
        t.is(statement.sql, sqlServerQuery(`CONCAT(${fixture.sqlServer}, '')`))
        t.deepEqual(statement.columns, columns(fixture.nullable))
        t.deepEqual(statement.bindings, [])
      }
      const legacy = graph.compileSqlServer({}, { version: '2008' })
      const oracle = graph.compileOracle({})
      t.is(legacy.sql, sqlServerQuery(`(COALESCE(${fixture.sqlServer}, N''))`))
      t.is(oracle.sql, oracleQuery(`(${fixture.oracle})`))
      t.deepEqual(legacy.columns, columns(fixture.nullable))
      t.deepEqual(oracle.columns, legacy.columns)
    }
  })

  test(`preserves existing SQL for concat with 2 and 254 arguments through ${authoring}`, (t) => {
    for (const count of [2, 254]) {
      const values = Array.from({ length: count }, (_, index) => `v${String(index).padStart(4, '0')}`)
      const graph = concatGraph(
        values.map((value) => literal(value)),
        authoring,
      )
      for (const version of ['2012', '2022'] as const) {
        const statement = graph.compileSqlServer({}, { version })
        t.is(statement.sql, sqlServerQuery(`CONCAT(${values.map((value) => `N'${value}'`).join(', ')})`))
        t.deepEqual(statement.columns, columns(false))
        t.deepEqual(statement.bindings, [])
      }
      t.is(
        graph.compileSqlServer({}, { version: '2008' }).sql,
        sqlServerQuery(`(${values.map((value) => `COALESCE(N'${value}', N'')`).join(' + ')})`),
      )
      t.is(graph.compileOracle({}).sql, oracleQuery(`(${values.map((value) => `'${value}'`).join(' || ')})`))
    }
  })

  test(`groups concat beyond 254 arguments without losing argument order through ${authoring}`, (t) => {
    for (const count of [255, 1000]) {
      const values = Array.from({ length: count }, (_, index) => `v${String(index).padStart(4, '0')}`)
      const graph = concatGraph(
        values.map((value) => literal(value)),
        authoring,
      )
      for (const version of ['2012', '2022'] as const) {
        const statement = graph.compileSqlServer({}, { version })
        const leafCount = Math.ceil(count / 253)
        t.is((statement.sql.match(/CONCAT\(CAST\(N'' AS nvarchar\(max\)\), /g) ?? []).length, leafCount)
        t.is((statement.sql.match(/CONCAT\(/g) ?? []).length, leafCount + 1)
        t.deepEqual(
          [...statement.sql.matchAll(/N'(v\d{4})'/g)].map((match) => match[1]),
          values,
        )
        t.deepEqual(statement.columns, columns(false))
        t.deepEqual(statement.bindings, [])
      }
      t.is(
        graph.compileSqlServer({}, { version: '2008' }).sql,
        sqlServerQuery(`(${values.map((value) => `COALESCE(N'${value}', N'')`).join(' + ')})`),
      )
      t.is(graph.compileOracle({}).sql, oracleQuery(`(${values.map((value) => `'${value}'`).join(' || ')})`))
    }
  })
}

test('preserves binding encounter order and repeated parameters across concat groups', (t) => {
  const first = requiredParameter('first', 'string')
  const second = requiredParameter('second', 'string')
  const args: Expression[] = Array.from({ length: 255 }, (_, index) => literal(`v${index}`))
  args[0] = param(second)
  args[1] = literal(null)
  args[2] = item.field('name')
  args[100] = param(first)
  args[252] = param(second)
  args[253] = param(first)
  args[254] = param(second)
  const graph = concatGraph(args, 'dsl', [first, second])
  const operation = { parameters: { first: 'A', second: 'B' } }
  const statements = [
    ...(['2008', '2012', '2022'] as const).map((version) => graph.compileSqlServer(operation, { version })),
    graph.compileOracle(operation),
  ]
  for (const statement of statements) {
    t.deepEqual(statement.bindings, [
      { name: 'p0', parameter: 'second', scalarType: 'string' },
      { name: 'p1', parameter: 'first', scalarType: 'string' },
    ])
    t.deepEqual(
      statement.sql.match(/[@:]p\d+/g)?.map((placeholder) => placeholder.slice(1)),
      ['p0', 'p1', 'p0', 'p1', 'p0'],
    )
    t.deepEqual(statement.columns, columns(false))
  }
})
