import test from 'ava'

import {
  and,
  coalesce,
  constraint,
  defineGraph,
  eq,
  exists,
  isNull,
  literal,
  neq,
  not,
  nullable,
  or,
  param,
  project,
  relation,
  requiredParameter,
  source,
} from '../definition.js'
import type { QueryGraphError, ResultOf } from '../definition.js'
import { registerDefinition } from '../index.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const item = source('item', { id: 'int64', name: nullable('string'), enabled: nullable('boolean') })
const mapping = { sources: { item: { table: 'Item' } } }

test('preserves boolean result types and metadata for projections and nested arguments', (t) => {
  const needle = requiredParameter('needle', 'string')
  const flag = requiredParameter('flag', 'boolean')
  const matches = eq(item.field('name'), needle)
  const definition = defineGraph({
    name: 'booleanValues',
    root: item,
    sources: [item],
    parameters: [needle, flag],
    projection: [
      project('equal', matches, { default: true }),
      project('different', neq(item.field('name'), needle), { default: true }),
      project('both', and(matches, item.field('enabled')), { default: true }),
      project('either', or(matches, eq(item.field('id'), 1)), { default: true }),
      project('negated', not(matches), { default: true }),
      project('missing', isNull(item.field('name')), { default: true }),
      project('nestedEqual', eq(matches, flag), { default: true }),
      project('fallback', coalesce(matches, false), { default: true }),
      project('nullableFallback', coalesce(matches, item.field('enabled')), { default: true }),
      project('field', item.field('enabled'), { default: true }),
      project('parameter', param(flag), { default: true }),
      project('constant', literal(true), { default: true }),
    ],
  })
  type Actual = ResultOf<typeof definition>
  type Expected = {
    equal: boolean | null
    different: boolean | null
    both: boolean | null
    either: boolean | null
    negated: boolean | null
    missing: boolean
    nestedEqual: boolean | null
    fallback: boolean
    nullableFallback: boolean | null
    field: boolean | null
    parameter: boolean
    constant: boolean
  }
  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)
  const unsafeAssumption = (row: Actual): boolean => {
    // @ts-expect-error Converting a predicate to a SQL value must preserve possible UNKNOWN.
    return row.equal
  }
  t.is(typeof unsafeAssumption, 'function')

  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const operation = { parameters: { needle: 'Ada', flag: true } }
  for (const [statement, sqlServer] of [
    [graph.compileSqlServer(operation, { version: '2008' }), true],
    [graph.compileSqlServer(operation, { version: '2022' }), true],
    [graph.compileOracle(operation, { version: '11g' }), false],
  ] as const) {
    const q = (name: string) => (sqlServer ? `[${name}]` : `"${name}"`)
    const name = `${q('t0')}.${q('name')}`
    const id = `${q('t0')}.${q('id')}`
    const enabled = `${q('t0')}.${q('enabled')}`
    const needle = sqlServer ? '@p0' : ':p0'
    const flag = sqlServer ? '@p1' : ':p1'
    const equal = `(${name} = ${needle})`
    const asValue = (predicate: string) =>
      `(CASE WHEN ${predicate} THEN 1 WHEN NOT (${predicate}) THEN 0 ELSE NULL END)`
    const expressions = [
      asValue(equal),
      asValue(`(${name} <> ${needle})`),
      asValue(`(${equal} AND (${enabled} = 1))`),
      asValue(`(${equal} OR (${id} = 1))`),
      asValue(`(NOT ${equal})`),
      asValue(`(${name} IS NULL)`),
      asValue(`(${asValue(equal)} = ${flag})`),
      `COALESCE(${asValue(equal)}, 0)`,
      `COALESCE(${asValue(equal)}, ${enabled})`,
      enabled,
      flag,
      '1',
    ]
    t.is(
      statement.sql,
      `SELECT\n  ${expressions.map((expression, index) => `${expression} AS ${q(`c${index}`)}`).join(',\n  ')}\nFROM ${q('Item')}${sqlServer ? ' AS' : ''} ${q('t0')}`,
    )
    t.deepEqual(statement.bindings, [
      { name: 'p0', parameter: 'needle', scalarType: 'string' },
      { name: 'p1', parameter: 'flag', scalarType: 'boolean' },
    ])
    t.deepEqual(
      statement.columns,
      [
        ['equal', true],
        ['different', true],
        ['both', true],
        ['either', true],
        ['negated', true],
        ['missing', false],
        ['nestedEqual', true],
        ['fallback', false],
        ['nullableFallback', true],
        ['field', true],
        ['parameter', false],
        ['constant', false],
      ].map(([path, nullable], index) => ({
        name: `c${index}`,
        path,
        scalarType: 'boolean',
        nullable,
        relations: [],
      })),
    )
  }
})

test('renders boolean scalar constraints and logical operands as predicates', (t) => {
  const flag = requiredParameter('flag', 'boolean')
  const definition = defineGraph({
    name: 'booleanPredicates',
    root: item,
    sources: [item],
    parameters: [flag],
    constraints: [
      constraint(item.field('enabled')),
      constraint(param(flag)),
      constraint(literal(true)),
      constraint(literal(false)),
      constraint(coalesce(item.field('enabled'), false)),
      constraint(and(item.field('enabled'), flag)),
      constraint(or(item.field('enabled'), false)),
      constraint(not(item.field('enabled'))),
      constraint(and(eq(item.field('id'), 1), literal(null))),
      constraint(isNull(item.field('name'))),
    ],
    projection: [project('id', item.field('id'), { default: true })],
  })
  const resultIsInferred: Equal<ResultOf<typeof definition>, { id: number | string }> = true
  t.true(resultIsInferred)
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const operation = { parameters: { flag: true } }
  for (const [statement, sqlServer] of [
    [graph.compileSqlServer(operation, { version: '2008' }), true],
    [graph.compileSqlServer(operation, { version: '2022' }), true],
    [graph.compileOracle(operation, { version: '11g' }), false],
  ] as const) {
    const q = (name: string) => (sqlServer ? `[${name}]` : `"${name}"`)
    const enabled = `${q('t0')}.${q('enabled')}`
    const id = `${q('t0')}.${q('id')}`
    const parameter = sqlServer ? '@p0' : ':p0'
    const asPredicate = (value: string) => `(${value} = 1)`
    const predicates = [
      asPredicate(enabled),
      asPredicate(parameter),
      asPredicate('1'),
      asPredicate('0'),
      asPredicate(`COALESCE(${enabled}, 0)`),
      `(${asPredicate(enabled)} AND ${asPredicate(parameter)})`,
      `(${asPredicate(enabled)} OR ${asPredicate('0')})`,
      `(NOT ${asPredicate(enabled)})`,
      `((${id} = 1) AND ${asPredicate('NULL')})`,
      `(${q('t0')}.${q('name')} IS NULL)`,
    ]
    t.is(
      statement.sql,
      `SELECT\n  ${id} AS ${q('c0')}\nFROM ${q('Item')}${sqlServer ? ' AS' : ''} ${q('t0')}\nWHERE\n  ${predicates.join('\n  AND ')}`,
    )
    t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'flag', scalarType: 'boolean' }])
    t.deepEqual(statement.columns, [{ name: 'c0', path: 'id', scalarType: 'int64', nullable: false, relations: [] }])
  }
})

for (const scalarPredicates of [true, false]) {
  test(`renders ON and EXISTS predicates with ${scalarPredicates ? 'boolean values' : 'unchanged comparisons'}`, (t) => {
    const child = source('child', { id: 'int64', enabled: nullable('boolean') })
    const graph = registerDefinition(
      defineGraph({
        name: 'booleanRelations',
        root: item,
        sources: [item, child],
        relations: [
          relation(
            'child',
            item,
            child,
            scalarPredicates ? child.field('enabled') : eq(item.field('id'), child.field('id')),
          ),
        ],
        constraints: [
          constraint(
            exists(child, scalarPredicates ? coalesce(child.field('enabled'), false) : neq(child.field('id'), 0), {
              from: item,
            }),
          ),
        ],
        projection: [project('id', child.field('id'), { default: true })],
      }),
    ).withRelationalMapping({ sources: { item: { table: 'Item' }, child: { table: 'Child' } } })
    for (const [statement, sqlServer] of [
      [graph.compileSqlServer({}), true],
      [graph.compileOracle({}), false],
    ] as const) {
      const q = (name: string) => (sqlServer ? `[${name}]` : `"${name}"`)
      const id = `${q('t1')}.${q('id')}`
      const enabled = `${q('t1')}.${q('enabled')}`
      const asPredicate = (value: string) => `(${value} = 1)`
      const on = scalarPredicates ? asPredicate(enabled) : `(${q('t0')}.${q('id')} = ${id})`
      const predicate = scalarPredicates ? asPredicate(`COALESCE(${enabled}, 0)`) : `(${id} <> 0)`
      const alias = sqlServer ? ' AS' : ''
      t.is(
        statement.sql,
        `SELECT\n  ${id} AS ${q('c0')}\nFROM ${q('Item')}${alias} ${q('t0')}\nLEFT JOIN ${q('Child')}${alias} ${q('t1')}\n  ON ${on}\nWHERE\n  EXISTS (\n    SELECT 1\n    FROM ${q('Child')}${alias} ${q('t1')}\n    WHERE ${on}\n      AND ${predicate}\n  )`,
      )
      t.deepEqual(statement.bindings, [])
      t.deepEqual(statement.columns, [
        { name: 'c0', path: 'id', scalarType: 'int64', nullable: true, relations: ['child'] },
      ])
    }
  })
}

test('preserves Oracle 23ai native predicate values and converts scalar predicate inputs', (t) => {
  const needle = requiredParameter('needle', 'string')
  const flag = requiredParameter('flag', 'boolean')
  const graph = registerDefinition(
    defineGraph({
      name: 'nativeBooleanValues',
      root: item,
      sources: [item],
      parameters: [needle, flag],
      constraints: [
        constraint(item.field('enabled')),
        constraint(param(flag)),
        constraint(eq(item.field('enabled'), true)),
      ],
      projection: [project('matches', eq(item.field('name'), needle), { default: true })],
    }),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters: { needle: 'Ada', flag: true } }, { version: '23ai' })
  t.is(
    statement.sql,
    'SELECT\n  ("t0"."name" = :p0) AS "c0"\nFROM "Item" "t0"\nWHERE\n  TO_BOOLEAN("t0"."enabled")\n  AND TO_BOOLEAN(:p1)\n  AND ("t0"."enabled" = 1)',
  )
  t.deepEqual(statement.bindings, [
    { name: 'p0', parameter: 'needle', scalarType: 'string' },
    { name: 'p1', parameter: 'flag', scalarType: 'boolean' },
  ])
  t.deepEqual(statement.columns, [
    { name: 'c0', path: 'matches', scalarType: 'boolean', nullable: true, relations: [] },
  ])
})

test('preserves definition diagnostics for a standalone null predicate', (t) => {
  const error = t.throws(() =>
    registerDefinition(
      defineGraph({
        name: 'invalidNullPredicate',
        root: item,
        sources: [item],
        constraints: [constraint(literal(null))],
      }),
    ),
  ) as QueryGraphError
  t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
  t.is(error.phase, 'definition')
  t.deepEqual(
    error.issues.map(({ code, location }) => ({ code, location })),
    [{ code: 'invalidPredicateType', location: 'constraints[0].predicate' }],
  )
})
