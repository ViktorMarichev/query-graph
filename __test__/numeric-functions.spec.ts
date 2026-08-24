import test from 'ava'

import { defineGraph, divide, modulo, nullable, project, source, truncate } from '../dsl.js'
import type { QueryGraphError, ResultOf } from '../dsl.js'
import { registerDefinition } from '../index.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const service = source('service', {
  durationMinutes: nullable('int64'),
})

const durationDefinition = defineGraph({
  name: 'serviceDuration',
  root: service,
  sources: [service],
  projection: [
    project({
      path: 'hoursDuration',
      expression: truncate(divide(service.field('durationMinutes'), 60)),
      default: true,
    }),
    project({
      path: 'minutesDuration',
      expression: modulo(service.field('durationMinutes'), 60),
      default: true,
    }),
  ],
})

test('builds portable duration expressions and infers their result', (t) => {
  type Actual = ResultOf<typeof durationDefinition>
  type Expected = {
    hoursDuration: number | string | null
    minutesDuration: number | string | null
  }

  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)
  t.deepEqual(
    durationDefinition.projection.fields.map((field) => field.expression),
    [
      {
        kind: 'function',
        name: 'truncate',
        arguments: [
          {
            kind: 'function',
            name: 'divide',
            arguments: [
              { kind: 'field', source: 'service', field: 'durationMinutes' },
              { kind: 'literal', value: { kind: 'integer', value: 60 } },
            ],
          },
        ],
      },
      {
        kind: 'function',
        name: 'modulo',
        arguments: [
          { kind: 'field', source: 'service', field: 'durationMinutes' },
          { kind: 'literal', value: { kind: 'integer', value: 60 } },
        ],
      },
    ],
  )
})

test('compiles numeric expressions to Oracle and SQL Server', (t) => {
  const graph = registerDefinition(durationDefinition).withRelationalMapping({
    sources: {
      service: { table: 'BusinessService' },
    },
  })

  const oracle = graph.compileOracle({})
  const sqlServer = graph.compileSqlServer({})

  t.deepEqual(oracle.columns, [
    { name: 'c0', path: 'hoursDuration', scalarType: 'decimal', nullable: true, relations: [] },
    { name: 'c1', path: 'minutesDuration', scalarType: 'int64', nullable: true, relations: [] },
  ])
  t.deepEqual(sqlServer.columns, oracle.columns)
  t.regex(oracle.sql, /TRUNC\(\("t0"\."durationMinutes" \/ 60\)\) AS "c0"/)
  t.regex(oracle.sql, /MOD\("t0"\."durationMinutes", 60\) AS "c1"/)
  t.regex(sqlServer.sql, /ROUND\(\(\(1\.0 \* \[t0\]\.\[durationMinutes\]\) \/ 60\), 0, 1\) AS \[c0\]/)
  t.regex(sqlServer.sql, /\(\[t0\]\.\[durationMinutes\] % 60\) AS \[c1\]/)
})

test('rejects modulo for float64 to keep definitions portable', (t) => {
  const measurement = source('measurement', {
    value: 'float64',
  })
  const definition = defineGraph({
    name: 'invalidFloatModulo',
    root: measurement,
    sources: [measurement],
    projection: [project({ path: 'remainder', expression: modulo(measurement.field('value'), 2), default: true })],
  })

  const error = t.throws(() => registerDefinition(definition)) as QueryGraphError

  t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
  t.true(error.issues.some((issue) => issue.code === 'invalidExpressionType'))
})
