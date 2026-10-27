import test from 'ava'

import {
  and,
  asc,
  constraint,
  defineGraph,
  desc,
  eq,
  inList,
  inParam,
  not,
  nullable,
  or,
  ordering,
  project,
  requiredListParam,
  requiredParam,
  source,
} from '../dsl.js'
import type { QueryGraphError } from '../dsl.js'
import { registerDefinition } from '../index.js'

const rows = source('rows', { id: 'int64', tenantId: 'int64', nullableId: nullable('int64') })
const ids = requiredListParam('ids', 'int64')
const mapping = { sources: { rows: { table: 'Rows' } } }
const selectId = 'SELECT\n  "t0"."id" AS "c0"\nFROM "Rows" "t0"'

function membershipSql(expression: string, values: readonly string[], limit = 1000): string {
  if (values.length === 0) return '(1 = 0)'
  const groups: string[] = []
  for (let offset = 0; offset < values.length; offset += limit) {
    groups.push(`(${expression} IN (${values.slice(offset, offset + limit).join(', ')}))`)
  }
  return groups.length === 1 ? groups[0] : `(${groups.join(' OR ')})`
}

function inGroupSizes(sql: string): number[] {
  return [...sql.matchAll(/ IN \(([^)]*)\)/g)].map((match) => match[1].split(', ').length)
}

for (const size of [0, 1, 1000, 1001, 2001]) {
  for (const negated of [false, true]) {
    test(`Oracle 19c ${negated ? 'NOT IN' : 'IN'} list parameter preserves ${size} bindings`, (t) => {
      const membership = inParam(rows.field('id'), ids)
      const graph = registerDefinition(
        defineGraph({
          name: 'parameterMembership',
          root: rows,
          sources: [rows],
          parameters: [ids],
          constraints: [constraint({ predicate: negated ? not(membership) : membership })],
          projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
        }),
      ).withRelationalMapping(mapping)
      const values = Array.from({ length: size }, (_, index) => index)
      const statement = graph.compileOracle({ parameters: { ids: values } }, { version: '19c' })
      const expected = membershipSql(
        '"t0"."id"',
        values.map((_, index) => `:p${index}`),
      )

      const expectedSizes = size === 0 ? [] : size === 1001 ? [1000, 1] : size === 2001 ? [1000, 1000, 1] : [size]
      t.deepEqual(inGroupSizes(statement.sql), expectedSizes)
      t.is(statement.sql, `${selectId}\nWHERE\n  ${negated ? `(NOT ${expected})` : expected}`)
      t.deepEqual(
        statement.bindings,
        values.map((_, index) => ({ name: `p${index}`, parameter: 'ids', scalarType: 'int64', index })),
      )
    })

    if (size === 0) continue

    test(`Oracle 19c ${negated ? 'NOT IN' : 'IN'} literal list preserves ${size} values`, (t) => {
      const values = Array.from({ length: size }, (_, index) => index)
      const membership = inList(rows.field('id'), values)
      const graph = registerDefinition(
        defineGraph({
          name: 'literalMembership',
          root: rows,
          sources: [rows],
          constraints: [constraint({ predicate: negated ? not(membership) : membership })],
          projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
        }),
      ).withRelationalMapping(mapping)
      const statement = graph.compileOracle({}, { version: '19c' })
      const expected = membershipSql('"t0"."id"', values.map(String))

      t.true(inGroupSizes(statement.sql).every((count) => count <= 1000))
      t.is(statement.sql, `${selectId}\nWHERE\n  ${negated ? `(NOT ${expected})` : expected}`)
      t.deepEqual(statement.bindings, [])
    })
  }
}

test('preserves validation of empty literal lists and null int64 list parameters', (t) => {
  for (const negated of [false, true]) {
    const membership = inList(rows.field('id'), [])
    const error = t.throws(() =>
      registerDefinition(
        defineGraph({
          name: 'emptyLiteralMembership',
          root: rows,
          sources: [rows],
          constraints: [constraint({ predicate: negated ? not(membership) : membership })],
          projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
        }),
      ),
    ) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
    t.is(error.issues[0].code, 'emptyExpressionGroup')
  }

  const graph = registerDefinition(
    defineGraph({
      name: 'nullListParameter',
      root: rows,
      sources: [rows],
      parameters: [ids],
      constraints: [constraint({ predicate: inParam(rows.field('nullableId'), ids) })],
      projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
    }),
  ).withRelationalMapping(mapping)
  const values = [...Array.from({ length: 1000 }, (_, index) => index), null]
  // Приведение проверяет контракт нетипизированных JavaScript-вызовов.
  const error = t.throws(() => graph.compileOracle({ parameters: { ids: values as never } })) as QueryGraphError
  t.is(error.code, 'QUERY_GRAPH_OPERATION_INVALID')
  t.deepEqual(
    error.issues.map(({ code, location }) => ({ code, location })),
    [{ code: 'invalidParameterType', location: 'parameters.ids[1000]' }],
  )
})

test('uses Oracle version limits without changing SQL Server list rendering', (t) => {
  const values = Array.from({ length: 2001 }, (_, index) => index)
  for (const literalValues of [false, true]) {
    for (const negated of [false, true]) {
      const membership = literalValues ? inList(rows.field('id'), values) : inParam(rows.field('id'), ids)
      const graph = registerDefinition(
        defineGraph({
          name: 'versionedMembership',
          root: rows,
          sources: [rows],
          parameters: [ids],
          constraints: [constraint({ predicate: negated ? not(membership) : membership })],
          projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
        }),
      ).withRelationalMapping(mapping)
      const operation = { parameters: { ids: values } }
      for (const version of ['11g', '12c', '19c', '21c'] as const) {
        t.deepEqual(inGroupSizes(graph.compileOracle(operation, { version }).sql), [1000, 1000, 1])
      }
      t.deepEqual(inGroupSizes(graph.compileOracle(operation).sql), [1000, 1000, 1])
      t.deepEqual(inGroupSizes(graph.compileOracle(operation, { version: '23ai' }).sql), [2001])
      t.deepEqual(inGroupSizes(graph.compileSqlServer(operation).sql), [2001])
    }
  }
})

test('preserves nested restrictions, repeated parameters, duplicate IDs, ordering, and pagination', (t) => {
  const excluded = requiredListParam('excluded', 'int64')
  const probe = requiredParam('probe', 'int64')
  const tenant = requiredParam('tenant', 'int64')
  const graph = registerDefinition(
    defineGraph({
      name: 'nestedMembership',
      root: rows,
      sources: [rows],
      parameters: [ids, excluded, probe, tenant],
      constraints: [
        constraint({
          predicate: and(
            inParam(probe, ids),
            or(not(inParam(rows.field('id'), excluded)), eq(rows.field('tenantId'), tenant)),
          ),
        }),
        constraint({ predicate: or(inParam(rows.field('id'), ids), eq(rows.field('id'), probe)) }),
        constraint({ predicate: eq(rows.field('tenantId'), tenant) }),
      ],
      projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
      orderings: [
        ordering({ name: 'idDesc', by: [desc(rows.field('id')), asc(rows.field('tenantId'))], default: true }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const parameters = {
    ids: Array.from({ length: 2001 }, (_, index) => index % 17),
    excluded: Array.from({ length: 1001 }, (_, index) => index % 13),
    probe: 0,
    tenant: 42,
  }
  const originalParameters = structuredClone(parameters)
  const statement = graph.compileOracle({ parameters, offset: 7, limit: 13 }, { version: '19c' })
  const idBindings = parameters.ids.map((_, index) => `:p${index + 1}`)
  const excludedBindings = parameters.excluded.map((_, index) => `:p${index + 2002}`)
  const first = membershipSql(':p0', idBindings)
  const second = membershipSql('"t0"."id"', excludedBindings)
  const repeated = membershipSql('"t0"."id"', idBindings)

  t.is(
    statement.sql,
    `${selectId}\nWHERE\n  (${first} AND ((NOT ${second}) OR ("t0"."tenantId" = :p3003)))\n  AND (${repeated} OR ("t0"."id" = :p0))\n  AND ("t0"."tenantId" = :p3003)\nORDER BY\n  "t0"."id" DESC,\n  "t0"."tenantId" ASC\nOFFSET 7 ROWS FETCH NEXT 13 ROWS ONLY`,
  )
  t.deepEqual(statement.bindings, [
    { name: 'p0', parameter: 'probe', scalarType: 'int64' },
    ...parameters.ids.map((_, index) => ({ name: `p${index + 1}`, parameter: 'ids', scalarType: 'int64', index })),
    ...parameters.excluded.map((_, index) => ({
      name: `p${index + 2002}`,
      parameter: 'excluded',
      scalarType: 'int64',
      index,
    })),
    { name: 'p3003', parameter: 'tenant', scalarType: 'int64' },
  ])
  t.deepEqual(parameters, originalParameters)
  t.deepEqual(statement.columns, [{ name: 'c0', path: 'id', scalarType: 'int64', nullable: false, relations: [] }])
})

test('keeps nullable IN and its negation as three-valued predicates across chunks', (t) => {
  const values = [...Array.from({ length: 1000 }, (_, index) => index % 17), null]
  for (const negated of [false, true]) {
    const membership = inList(rows.field('nullableId'), values)
    const predicate = negated ? not(membership) : membership
    const graph = registerDefinition(
      defineGraph({
        name: 'nullableMembership',
        root: rows,
        sources: [rows],
        constraints: [constraint({ predicate })],
        projection: [project({ path: 'matches', expression: predicate, default: true })],
      }),
    ).withRelationalMapping(mapping)
    const statement = graph.compileOracle({}, { version: '19c' })
    const membershipExpression = membershipSql(
      '"t0"."nullableId"',
      values.map((value) => (value === null ? 'NULL' : String(value))),
    )
    const expected = negated ? `(NOT ${membershipExpression})` : membershipExpression

    t.is(
      statement.sql,
      `SELECT\n  (CASE WHEN ${expected} THEN 1 WHEN NOT (${expected}) THEN 0 ELSE NULL END) AS "c0"\nFROM "Rows" "t0"\nWHERE\n  ${expected}`,
    )
    t.deepEqual(statement.bindings, [])
    t.deepEqual(statement.columns, [
      { name: 'c0', path: 'matches', scalarType: 'boolean', nullable: true, relations: [] },
    ])
  }
})

test('reuses scalar bindings in large inList expressions', (t) => {
  const probe = requiredParam('probe', 'int64')
  const values = [...Array.from({ length: 1000 }, (_, index) => index), probe]
  const graph = registerDefinition(
    defineGraph({
      name: 'literalScalarMembership',
      root: rows,
      sources: [rows],
      parameters: [probe],
      constraints: [constraint({ predicate: inList(probe, values) })],
      projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
    }),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters: { probe: 0 } }, { version: '19c' })
  const expected = membershipSql(':p0', [...Array.from({ length: 1000 }, (_, index) => String(index)), ':p0'])

  t.is(statement.sql, `${selectId}\nWHERE\n  ${expected}`)
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'probe', scalarType: 'int64' }])
})
