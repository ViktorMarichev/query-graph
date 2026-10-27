// @ts-expect-error The DSL type-test project does not include Node runtime declarations.
import { createRequire } from 'node:module'
import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  allGroups,
  and,
  asc,
  batchQuery,
  batchRelation,
  composeGraph,
  constraint,
  count,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  eq,
  exists,
  inList,
  inParam,
  inParameter,
  isNull,
  measure,
  not,
  nullable,
  optionalGroupsParam,
  optionalGroupsParameter,
  optionalParam,
  or,
  ordering,
  param,
  project,
  relation,
  requiredGroupsParam,
  requiredGroupsParameter,
  requiredListParam,
  requiredParam,
  source,
} from '../dsl.js'
import type {
  Expression,
  ExpressionNullability,
  ExpressionScalarType,
  GroupScope,
  GroupValuesRef,
  ParameterValue,
  QueryGraphError,
  QueryOperation,
  ScalarParameterValue,
} from '../dsl.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const rows = source('rows', { id: 'int64', kind: 'string', nullableKind: nullable('string') })
const groups = optionalGroupsParam('groups', { key: 'string', value: 'int64' })
const base = {
  name: 'groupedRows',
  root: rows,
  sources: [rows],
  projection: [project({ path: 'id', expression: rows.field('id'), default: true })],
}
const mapping = { sources: { rows: { table: 'Rows' } } }
const methods = ['compileSqlServer', 'compileOracle'] as const

function groupedPredicate() {
  return allGroups(groups, ({ key, values }) => and(eq(rows.field('kind'), key), inParam(rows.field('id'), values)))
}

test('authoring: grouped factories preserve value types and aliases in version 12', (t) => {
  const required = requiredGroupsParameter('required', { key: 'int32', value: 'string' })
  t.is(optionalGroupsParam, optionalGroupsParameter)
  t.is(requiredGroupsParam, requiredGroupsParameter)
  t.deepEqual(groups, { name: 'groups', scalarType: 'int64', shape: 'groupedList', keyType: 'string' })
  t.deepEqual(required, {
    name: 'required',
    scalarType: 'string',
    required: true,
    shape: 'groupedList',
    keyType: 'int32',
  })
  const definition = defineGraph({
    ...base,
    parameters: [groups],
    constraints: [constraint({ predicate: groupedPredicate() })],
  })
  t.is(definition.schemaVersion, 12)
  t.deepEqual(JSON.parse(JSON.stringify(definition.parameters)), [groups])
  t.true(Object.isFrozen(definition.constraints[0].predicate))
})

test('authoring: grouped factories reject unknown keys, scalar types and invalid inputs', (t) => {
  for (const factory of [optionalGroupsParameter, requiredGroupsParameter]) {
    for (const name of ['', ' ', null, 1]) {
      t.throws(() => factory(name as never, { key: 'string', value: 'int64' }), { instanceOf: TypeError })
    }
    for (const types of [
      undefined,
      null,
      [],
      'string',
      {},
      { key: 'string' },
      { value: 'int64' },
      { key: 'unknown', value: 'int64' },
      { key: 'string', value: 'unknown' },
      { key: null, value: 'int64' },
      { key: 'string', value: ['int64'] },
      { key: 'string', value: 'int64', extra: true },
      { key: 'string', value: 'int64', [Symbol('extra')]: true },
    ]) {
      t.throws(() => factory('invalid', types as never), { instanceOf: TypeError })
    }
  }
  for (const reference of [
    undefined,
    null,
    'groups',
    groups.name,
    {},
    requiredParam('p', 'int64'),
    requiredListParam('p', 'int64'),
    { ...groups },
  ]) {
    t.throws(() => allGroups(reference as never, () => true), { instanceOf: TypeError })
  }
  for (const predicate of [undefined, null, true, {}, []]) {
    t.throws(() => allGroups(groups, predicate as never), { instanceOf: TypeError })
  }
  for (const result of [undefined, {}, [], Promise.resolve(true)]) {
    t.throws(() => allGroups(groups, () => result as never), { instanceOf: TypeError })
  }
  t.throws(
    () =>
      allGroups(groups, () => {
        throw new Error('callback failed')
      }),
    { message: 'callback failed' },
  )
})

test('authoring: callbacks run once and nested binders preserve captured references', (t) => {
  let calls = 0
  let captured: GroupScope<'string', 'int64'> | undefined
  const outer = allGroups(groups, (outerGroup) => {
    calls++
    captured = outerGroup
    return allGroups(groups, ({ key, values }) => {
      calls++
      return and(
        eq(key, outerGroup.key),
        inParam(rows.field('id'), outerGroup.values),
        inParameter(rows.field('id'), values),
      )
    })
  })
  const sibling = allGroups(groups, ({ key }) => eq(rows.field('kind'), key))
  t.is(calls, 2)
  t.is(outer.kind, 'allGroups')
  t.is(outer.parameter, 'groups')
  const inner = outer.predicate
  if (inner.kind !== 'allGroups') return t.fail('Expected nested allGroups')
  t.is(new Set([outer.scope, inner.scope, sibling.scope]).size, 3)
  t.true([outer.scope, inner.scope, sibling.scope].every((scope) => typeof scope === 'string' && scope.length > 0))
  t.deepEqual(inner.predicate, {
    kind: 'and',
    expressions: [
      { kind: 'eq', left: { kind: 'groupKey', scope: inner.scope }, right: { kind: 'groupKey', scope: outer.scope } },
      { kind: 'inGroupValues', expression: rows.field('id'), scope: outer.scope },
      { kind: 'inGroupValues', expression: rows.field('id'), scope: inner.scope },
    ],
  })
  t.deepEqual(captured?.key, { kind: 'groupKey', scope: outer.scope })
  t.true(Object.isFrozen(captured?.values))
  t.deepEqual(JSON.parse(JSON.stringify(outer)), outer)
})

test.serial('runtime: fresh authoring modules cannot capture escaped group keys', (t) => {
  const require = createRequire(import.meta.url)
  const modulePath = require.resolve('../lib/authoring/primitives.js')
  const cached = require.cache[modulePath]
  type Primitives = Pick<typeof import('../dsl.js'), 'allGroups' | 'optionalGroupsParam'>
  try {
    delete require.cache[modulePath]
    const first = require(modulePath) as Primitives
    delete require.cache[modulePath]
    const second = require(modulePath) as Primitives
    t.not(first, second)

    const firstParameter = first.optionalGroupsParam('groups', { key: 'string', value: 'int64' })
    let escapedKey: GroupScope<'string', 'int64'>['key'] | undefined
    const firstBinder = first.allGroups(firstParameter, ({ key }) => {
      escapedKey = key
      return eq(rows.field('kind'), key)
    })
    const secondParameter = second.optionalGroupsParam('groups', { key: 'string', value: 'int64' })
    const secondBinder = second.allGroups(secondParameter, () => eq(rows.field('kind'), escapedKey!))
    const error = t.throws(() =>
      registerDefinition(
        defineGraph({
          ...base,
          parameters: [secondParameter],
          constraints: [constraint({ predicate: secondBinder })],
        }),
      ),
    ) as QueryGraphError
    t.is(error.phase, 'definition')
    t.true(error.issues.some(({ code }) => code === 'unknownGroupScope'))
    t.not(firstBinder.scope, secondBinder.scope)
  } finally {
    if (cached) require.cache[modulePath] = cached
    else delete require.cache[modulePath]
  }
  t.is(require.cache[modulePath], cached)
})

test('authoring: group values are neither scalar expressions nor root parameters', (t) => {
  t.throws(() => param(groups as never), { message: /Grouped list parameter groups/ })
  t.throws(() => eq(rows.field('id'), groups as never), { instanceOf: TypeError })
  t.throws(() => inParam(rows.field('id'), groups as never), { instanceOf: TypeError })
  allGroups(groups, ({ values }) => {
    for (const action of [
      () => param(values as never),
      () => eq(rows.field('id'), values as never),
      () => project({ path: 'values', expression: values as never }),
      () => inList(rows.field('id'), [values as never]),
      () => inParam(values as never, values),
      () => constraint({ predicate: eq(rows.field('id'), 1), when: values as never }),
      () => allGroups(values as never, () => true),
      () => allGroups(groups, () => values as never),
      () => inParam(rows.field('id'), { ...values } as never),
    ])
      t.throws(action, { instanceOf: TypeError })
    return true
  })
})

test('authoring: TypeScript infers grouped operations, scoped key types and predicate nullability', (t) => {
  const required = requiredGroupsParam('required', { key: 'int32', value: 'string' })
  const json = optionalGroupsParam('json', { key: 'json', value: 'json' })
  const module = defineGraphModule({ name: 'groupModule', parameters: [required] })
  const definition = defineGraph({ ...base, modules: [module], parameters: [groups, json] })
  type Operation = QueryOperation<typeof definition>
  const operation = {
    parameters: {
      required: [{ key: 42, values: ['first', 'second'] }],
      groups: [{ key: 'kind', values: [1, '9223372036854775807'] }],
      json: [{ key: null, values: [null, { nested: [true, 1, 'text'] }] }],
    },
  } as const satisfies Operation
  const optionalOnly: QueryOperation<ReturnType<typeof optionalDefinition>> = {}
  function optionalDefinition() {
    return defineGraph({ ...base, parameters: [groups] })
  }
  const inferred: Equal<
    ParameterValue<typeof groups>,
    readonly { key: string; values: readonly (number | string)[] }[]
  > = true
  const jsonInferred: Equal<
    ParameterValue<typeof json>,
    readonly { key: ScalarParameterValue<'json'>; values: readonly ScalarParameterValue<'json'>[] }[]
  > = true
  t.true(inferred)
  t.true(jsonInferred)
  t.truthy(operation)
  t.deepEqual(optionalOnly, {})

  const predicate = eq(rows.field('nullableKind'), 'test')
  const quantified = allGroups(groups, ({ key, values }) => {
    const keyType: Equal<ExpressionScalarType<typeof key>, 'string'> = true
    const keyNullability: Equal<ExpressionNullability<typeof key>, false> = true
    const valueType: Equal<typeof values, GroupValuesRef<'int64'>> = true
    t.true(keyType)
    t.true(keyNullability)
    t.true(valueType)
    return predicate
  })
  const preservedNullability: Equal<
    ExpressionNullability<typeof quantified>,
    ExpressionNullability<typeof predicate>
  > = true
  const quantifiedType: Equal<ExpressionScalarType<typeof quantified>, 'boolean'> = true
  const nonnullable = allGroups(groups, () => exists(rows))
  const existsNullability: Equal<ExpressionNullability<typeof nonnullable>, false> = true
  t.true(preservedNullability)
  t.true(quantifiedType)
  t.true(existsNullability)

  const invalidTypeUsage = (group: GroupScope<'string', 'int64'>) => {
    // @ts-expect-error Required grouped parameters make parameters mandatory.
    const missing: Operation = {}
    // @ts-expect-error A required group cannot be omitted.
    const omitted: Operation = { parameters: {} }
    // @ts-expect-error Grouped containers cannot be null.
    const nullContainer: Operation = { parameters: { required: null } }
    // @ts-expect-error Individual groups cannot be null.
    const nullGroup: Operation = { parameters: { required: [null] } }
    // @ts-expect-error Non-JSON keys cannot be null.
    const nullKey: Operation = { parameters: { required: [{ key: null, values: [] }] } }
    // @ts-expect-error Values containers cannot be null.
    const nullValues: Operation = { parameters: { required: [{ key: 1, values: null }] } }
    // @ts-expect-error Non-JSON elements cannot be null.
    const nullElement: Operation = { parameters: { required: [{ key: 1, values: [null] }] } }
    // @ts-expect-error Keys must have the declared scalar type.
    const wrongKey: Operation = { parameters: { required: [{ key: '1', values: [] }] } }
    // @ts-expect-error Elements must have the declared scalar type.
    const wrongValue: Operation = { parameters: { required: [{ key: 1, values: [2] }] } }
    // @ts-expect-error Each group requires values.
    const missingValues: Operation = { parameters: { required: [{ key: 1 }] } }
    // @ts-expect-error Unknown group properties are rejected.
    const extra: Operation = { parameters: { required: [{ key: 1, values: [], extra: true }] } }
    // @ts-expect-error JSON groups still require array containers.
    const nullJson: Operation = { parameters: { required: [], json: null } }
    // @ts-expect-error Unknown factory options are rejected.
    optionalGroupsParam('bad', { key: 'string', value: 'int64', extra: true })
    // @ts-expect-error Unknown scalar types are rejected.
    optionalGroupsParam('bad', { key: 'number', value: 'int64' })
    // @ts-expect-error Grouped parameters are not scalars.
    param(groups)
    // @ts-expect-error Grouped parameters are not expressions.
    eq(rows.field('id'), groups)
    // @ts-expect-error Grouped parameters are not ordinary list parameters.
    inParam(rows.field('id'), groups)
    // @ts-expect-error A list parameter is not a grouped parameter.
    allGroups(requiredListParam('ids', 'int64'), () => true)
    // @ts-expect-error Group values are not scalars.
    param(group.values)
    // @ts-expect-error Group values are not scalar operands.
    eq(rows.field('id'), group.values)
    // @ts-expect-error Group values are not projection expressions.
    project({ path: 'values', expression: group.values })
    // @ts-expect-error Group values are not root parameters.
    defineGraph({ ...base, parameters: [group.values] })
    // @ts-expect-error Group values cannot be returned as a predicate.
    allGroups(groups, () => group.values)
    return [
      missing,
      omitted,
      nullContainer,
      nullGroup,
      nullKey,
      nullValues,
      nullElement,
      wrongKey,
      wrongValue,
      missingValues,
      extra,
      nullJson,
    ]
  }
  t.is(typeof invalidTypeUsage, 'function')
})

test('runtime: repeated keys are independent AND groups with path bindings', (t) => {
  const id = requiredParam('id', 'int64')
  const ids = requiredListParam('ids', 'int64')
  const predicate = groupedPredicate()
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [groups, id, ids],
      constraints: [
        constraint({ predicate: and(eq(rows.field('id'), id), inParam(rows.field('id'), ids), predicate, predicate) }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const operation = {
    parameters: {
      id: 1,
      ids: [1, 2],
      groups: [
        { key: 'same', values: [1, '2'] },
        { key: 'same', values: [3] },
      ],
    },
  }
  for (const method of methods) {
    const statement = graph[method](operation)
    t.deepEqual(statement.bindings, [
      { name: 'p0', parameter: 'id', scalarType: 'int64' },
      { name: 'p1', parameter: 'ids', scalarType: 'int64', index: 0 },
      { name: 'p2', parameter: 'ids', scalarType: 'int64', index: 1 },
      { name: 'p3', parameter: 'groups', scalarType: 'string', path: [0, 'key'] },
      { name: 'p4', parameter: 'groups', scalarType: 'int64', path: [0, 'values', 0] },
      { name: 'p5', parameter: 'groups', scalarType: 'int64', path: [0, 'values', 1] },
      { name: 'p6', parameter: 'groups', scalarType: 'string', path: [1, 'key'] },
      { name: 'p7', parameter: 'groups', scalarType: 'int64', path: [1, 'values', 0] },
    ])
    t.regex(statement.sql, / AND /)
    t.false(statement.sql.includes(' OR '))
    t.false(statement.sql.includes('same'))
    t.is((statement.sql.match(/[@:]p3\b/g) ?? []).length, 2)
    t.is((statement.sql.match(/[@:]p6\b/g) ?? []).length, 2)
    t.true(statement.bindings.every((binding) => !('index' in binding && 'path' in binding)))
  }
})

test('runtime: absent and empty groups do not require optional body parameters or joins', (t) => {
  const detail = source('detail', { id: 'int64', kind: 'string' })
  const probe = optionalParam('probe', 'string')
  const detailRelation = relation({
    name: 'detail',
    from: rows,
    to: detail,
    on: eq(rows.field('id'), detail.field('id')),
  })
  for (const useExists of [false, true]) {
    const graph = registerDefinition(
      defineGraph({
        ...base,
        sources: [rows, detail],
        parameters: [groups, probe],
        relations: [detailRelation],
        constraints: [
          constraint({
            predicate: allGroups(groups, ({ key, values }) => {
              const body = and(
                eq(detail.field('kind'), key),
                eq(detail.field('kind'), probe),
                inParam(detail.field('id'), values),
              )
              return useExists ? exists(detail, body) : body
            }),
          }),
        ],
      }),
    ).withRelationalMapping({ sources: { ...mapping.sources, detail: { table: 'Detail' } } })
    for (const method of methods) {
      const absent = graph[method]({})
      const empty = graph[method]({ parameters: { groups: [] } })
      t.is(absent.sql, empty.sql)
      for (const statement of [absent, empty]) {
        t.deepEqual(statement.bindings, [])
        t.deepEqual(statement.relations, [])
        t.false(statement.sql.includes('Detail'))
        t.false(statement.sql.includes('1 = 0'))
      }
      const error = t.throws(() =>
        graph[method]({ parameters: { groups: [{ key: 'x', values: [1] }] } }),
      ) as QueryGraphError
      t.is(error.phase, 'operation')
      t.true(error.issues.some(({ location }) => location === 'parameters.probe'))
      const active = graph[method]({ parameters: { groups: [{ key: 'x', values: [1] }], probe: 'x' } })
      t.true(active.sql.includes('Detail'))
    }
  }
})

test('runtime: empty group values yield false without unrendered key bindings', (t) => {
  const numeric = requiredGroupsParam('numeric', { key: 'int64', value: 'int64' })
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [numeric],
      constraints: [constraint({ predicate: allGroups(numeric, ({ key, values }) => inParam(key, values)) })],
    }),
  ).withRelationalMapping(mapping)
  for (const method of methods) {
    const empty = graph[method]({ parameters: { numeric: [{ key: 1, values: [] }] } })
    t.regex(empty.sql, /1 = 0/)
    t.deepEqual(empty.bindings, [])
    const noGroups = graph[method]({ parameters: { numeric: [] } })
    t.false(noGroups.sql.includes('1 = 0'))
    t.deepEqual(noGroups.bindings, [])
    t.throws(() => graph[method]({} as never))
  }
})

test('runtime: nested binders on the same parameter keep inner and captured paths separate', (t) => {
  const predicate = allGroups(groups, (outer) =>
    allGroups(groups, (inner) =>
      and(eq(outer.key, inner.key), inParam(rows.field('id'), outer.values), inParam(rows.field('id'), inner.values)),
    ),
  )
  const graph = registerDefinition(
    defineGraph({ ...base, parameters: [groups], constraints: [constraint({ predicate })] }),
  ).withRelationalMapping(mapping)
  for (const method of methods) {
    const statement = graph[method]({
      parameters: {
        groups: [
          { key: 'one', values: [1] },
          { key: 'two', values: [2] },
        ],
      },
    })
    t.is(statement.bindings.length, 4)
    for (const path of [
      [0, 'key'],
      [0, 'values', 0],
      [1, 'key'],
      [1, 'values', 0],
    ]) {
      t.is(
        statement.bindings.filter((binding) => JSON.stringify(Reflect.get(binding, 'path')) === JSON.stringify(path))
          .length,
        1,
      )
    }
    t.is((statement.sql.match(/ IN /g) ?? []).length, 8)
    t.false(statement.sql.includes(' OR '))
  }
})

test('runtime: malformed groups and scalar values fail with operation locations', (t) => {
  const graph = registerDefinition(
    defineGraph({ ...base, parameters: [groups], constraints: [constraint({ predicate: groupedPredicate() })] }),
  ).withRelationalMapping(mapping)
  const invalid: Array<[unknown, string]> = [
    [null, 'parameters.groups'],
    [{}, 'parameters.groups'],
    [[null], 'parameters.groups[0]'],
    [[[]], 'parameters.groups[0]'],
    [[{ key: null, values: [] }], 'parameters.groups[0].key'],
    [[{ key: 1, values: [] }], 'parameters.groups[0].key'],
    [[{ values: [] }], 'parameters.groups[0].key'],
    [[{ key: 'x' }], 'parameters.groups[0].values'],
    [[{ key: 'x', values: null }], 'parameters.groups[0].values'],
    [[{ key: 'x', values: 1 }], 'parameters.groups[0].values'],
    [[{ key: 'x', values: [null] }], 'parameters.groups[0].values[0]'],
    [[{ key: 'x', values: [false] }], 'parameters.groups[0].values[0]'],
    [[{ key: 'x', values: [1.5] }], 'parameters.groups[0].values[0]'],
    [[{ key: 'x', values: ['not-an-int'] }], 'parameters.groups[0].values[0]'],
    [[{ key: 'x', values: [], extra: 1 }], 'parameters.groups[0].extra'],
  ]
  for (const method of methods) {
    for (const [value, location] of invalid) {
      const error = t.throws(() => graph[method]({ parameters: { groups: value as never } })) as QueryGraphError
      t.is(error.phase, 'operation')
      t.true(
        error.issues.some((issue) => issue.location === location),
        location,
      )
    }
  }
})

test('runtime: JSON permits null keys and elements but never null group containers', (t) => {
  const json = requiredGroupsParam('json', { key: 'json', value: 'json' })
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [json],
      constraints: [constraint({ predicate: allGroups(json, ({ key }) => isNull(key)) })],
    }),
  ).withRelationalMapping(mapping)
  for (const method of methods) {
    const statement = graph[method]({ parameters: { json: [{ key: null, values: [null, { nested: [1, true] }] }] } })
    t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'json', scalarType: 'json', path: [0, 'key'] }])
    for (const value of [null, [null], [{ key: null, values: null }]]) {
      t.throws(() => graph[method]({ parameters: { json: value as never } }))
    }
  }
})

test('runtime: registration validates lexical scope and grouped expression types', (t) => {
  let captured: GroupScope<'string', 'int64'> | undefined
  const binder = allGroups(groups, (group) => {
    captured = group
    return true
  })
  if (!captured) return t.fail('Expected callback refs')
  const invalid: Expression[] = [
    eq(rows.field('kind'), captured.key),
    inParam(rows.field('id'), captured.values),
    allGroups(groups, () => eq(rows.field('kind'), captured!.key)),
    allGroups(groups, () => inParam(rows.field('id'), captured!.values)),
    allGroups(groups, ({ key }) => eq(rows.field('id'), key)),
    allGroups(groups, ({ values }) => inParam(rows.field('kind'), values)),
    allGroups(groups, ({ key }) => key),
    { ...binder, scope: '' },
    { ...binder, predicate: binder },
    { ...binder, parameter: 'missing' },
    { kind: 'parameter', name: groups.name },
    { kind: 'inParameter', expression: rows.field('id'), parameter: groups.name },
  ]
  for (const predicate of invalid) {
    const error = t.throws(() =>
      registerDefinition(defineGraph({ ...base, parameters: [groups], constraints: [constraint({ predicate })] })),
    ) as QueryGraphError
    t.is(error.phase, 'definition')
  }
})

test('runtime: allGroups is accepted in boolean/exists constraints but not projection, ordering, relation or aggregate bodies', (t) => {
  const detail = source('detail', { id: 'int64' })
  const edge = relation({ name: 'detail', from: rows, to: detail, on: eq(rows.field('id'), detail.field('id')) })
  const predicate = allGroups(groups, ({ values }) => inParam(detail.field('id'), values))
  t.notThrows(() =>
    registerDefinition(
      defineGraph({
        ...base,
        sources: [rows, detail],
        parameters: [groups],
        relations: [edge],
        constraints: [constraint({ predicate: not(or(exists(detail, predicate), groupedPredicate())) })],
      }),
    ),
  )
  const simple = groupedPredicate()
  for (const definition of [
    defineGraph({ ...base, parameters: [groups], projection: [project({ path: 'invalid', expression: simple })] }),
    defineGraph({ ...base, parameters: [groups], orderings: [ordering({ name: 'invalid', by: [asc(simple)] })] }),
    defineGraph({ ...base, sources: [rows, detail], parameters: [groups], relations: [{ ...edge, on: simple }] }),
    defineSummaryGraph({
      name: 'invalid',
      root: rows,
      sources: [rows],
      parameters: [groups],
      measures: [measure({ path: 'invalid', expression: count(simple) })],
    }),
  ]) {
    const error = t.throws(() => registerDefinition(definition)) as QueryGraphError
    t.is(error.phase, 'definition')
    t.true(error.issues.some(({ code }) => code === 'invalidAllGroupsContext'))
  }
})

test('runtime: Oracle chunks group values while SQL Server counts distinct grouped and legacy bindings', (t) => {
  const id = requiredParam('id', 'int64')
  const ids = requiredListParam('ids', 'int64')
  const predicate = groupedPredicate()
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [groups, id, ids],
      constraints: [
        constraint({ predicate: and(eq(rows.field('id'), id), inParam(rows.field('id'), ids), predicate, predicate) }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const operation = (size: number) => ({
    parameters: { id: 1, ids: [1], groups: [{ key: 'x', values: Array.from({ length: size }, (_, index) => index) }] },
  })
  const boundary = graph.compileSqlServer(operation(2097))
  t.is(boundary.bindings.length, 2100)
  const error = t.throws(() => graph.compileSqlServer(operation(2098))) as QueryGraphError
  t.true(error.issues.some(({ code, location }) => code === 'tooManyParameters' && location === 'operation.parameters'))
  const oracle = graph.compileOracle(operation(2098), { version: '19c' })
  t.is(oracle.bindings.length, 2101)
  const sizes = [...oracle.sql.matchAll(/ IN \(([^)]*)\)/g)].map((match) => match[1].split(', ').length)
  t.deepEqual(sizes, [1, 1000, 1000, 98, 1000, 1000, 98])
  t.deepEqual(oracle.bindings.at(-1), {
    name: 'p2100',
    parameter: 'groups',
    scalarType: 'int64',
    path: [0, 'values', 2097],
  })
})

test('runtime: total grouped expansion is bounded independently of distinct bindings and resets per operation', (t) => {
  const predicate = allGroups(groups, () =>
    allGroups(groups, () => allGroups(groups, ({ key }) => eq(rows.field('kind'), key))),
  )
  const graph = registerDefinition(
    defineGraph({ ...base, parameters: [groups], constraints: [constraint({ predicate })] }),
  ).withRelationalMapping(mapping)
  // Three levels expand 30 + 30^2 + 30^3 times, but only the 30 inner key paths are bound.
  const operation = {
    parameters: { groups: Array.from({ length: 30 }, (_, index) => ({ key: `kind${index}`, values: [] })) },
  }
  for (const method of methods) {
    const error = t.throws(() => graph[method](operation)) as QueryGraphError
    t.is(error.phase, 'sql')
    t.deepEqual(
      error.issues.map(({ code, location }) => ({ code, location })),
      [{ code: 'groupExpansionLimitExceeded', location: 'operation.parameters' }],
    )
    const next = graph[method]({ parameters: { groups: [{ key: 'small', values: [] }] } })
    t.deepEqual(next.bindings, [{ name: 'p0', parameter: 'groups', scalarType: 'string', path: [0, 'key'] }])
  }
})

test('runtime: repeated list expansion is bounded even when bindings are reused', (t) => {
  const inner = optionalGroupsParam('inner', { key: 'string', value: 'int64' })
  const ids = requiredListParam('ids', 'int64')
  const values = Array.from({ length: 1000 }, (_, index) => index)
  for (const captured of [false, true]) {
    const predicate = allGroups(groups, (outer) =>
      allGroups(inner, () => (captured ? inParam(rows.field('id'), outer.values) : inParam(rows.field('id'), ids))),
    )
    const graph = registerDefinition(
      defineGraph({ ...base, parameters: [groups, inner, ids], constraints: [constraint({ predicate })] }),
    ).withRelationalMapping(mapping)
    const operation = {
      parameters: {
        groups: [{ key: 'outer', values }],
        inner: Array.from({ length: 101 }, () => ({ key: 'inner', values: [] })),
        ids: values,
      },
    }
    for (const method of methods) {
      const error = t.throws(() => graph[method](operation)) as QueryGraphError
      t.is(error.phase, 'sql')
      t.deepEqual(
        error.issues.map(({ code, location }) => ({ code, location })),
        [{ code: 'groupValueExpansionLimitExceeded', location: 'operation.parameters' }],
      )
      t.is(graph[method]({ parameters: { ...operation.parameters, inner: [] } }).bindings.length, 0)
    }
  }
})

test('runtime: batch child static groups preserve path bindings but cannot serve as batch keys', (t) => {
  const ids = requiredListParam('ids', 'int64')
  const filters = requiredGroupsParam('filters', { key: 'string', value: 'int64' })
  const parent = registerDefinition(defineGraph({ ...base, name: 'parentRows' })).withRelationalMapping(mapping)
  const child = registerDefinition(
    defineGraph({
      ...base,
      parameters: [ids, filters],
      constraints: [
        constraint({
          predicate: and(
            inParam(rows.field('id'), ids),
            allGroups(filters, ({ key, values }) =>
              and(eq(rows.field('kind'), key), inParam(rows.field('id'), values)),
            ),
          ),
        }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const query = batchQuery({ graph: child, key: { path: 'id', parameter: ids } })
  const staticParameters = { filters: [{ key: 'kind', values: [1, '2'] }] } as const
  const relationConfiguration = { name: 'children', from: 'id', query, cardinality: 'many' } as const
  const children = batchRelation({ ...relationConfiguration, parameters: staticParameters })
  const graph = composeGraph({ root: parent, relations: [children] })
  for (const method of ['compileSqlServerPlan', 'compileOraclePlan'] as const) {
    const plan = graph[method]({ select: ['children.id'] })
    t.deepEqual(plan.root.bindings, [])
    t.deepEqual(plan.batches[0].parameters, staticParameters)
    const statement = plan.compileBatch('children', [1, 2])
    t.deepEqual(statement.bindings, [
      { name: 'p0', parameter: 'ids', scalarType: 'int64', index: 0 },
      { name: 'p1', parameter: 'ids', scalarType: 'int64', index: 1 },
      { name: 'p2', parameter: 'filters', scalarType: 'string', path: [0, 'key'] },
      { name: 'p3', parameter: 'filters', scalarType: 'int64', path: [0, 'values', 0] },
      { name: 'p4', parameter: 'filters', scalarType: 'int64', path: [0, 'values', 1] },
    ])
    t.true(statement.bindings.every((binding) => !('index' in binding && 'path' in binding)))
  }
  const invalidQuery = batchQuery({ graph: child, key: { path: 'id', parameter: filters as unknown as typeof ids } })
  const invalidRelation = batchRelation({
    ...relationConfiguration,
    query: invalidQuery,
    parameters: { ids: [1] },
  } as never)
  const error = t.throws(() => composeGraph({ root: parent, relations: [invalidRelation as never] })) as QueryGraphError
  t.is(error.phase, 'composition')
  const invalidTypes = () => {
    // @ts-expect-error A grouped reference cannot be the batch key list.
    batchQuery({ graph: child, key: { path: 'id', parameter: filters } })
    // @ts-expect-error A grouped parameter name cannot be the batch key list either.
    batchQuery({ graph: child, key: { path: 'id', parameter: 'filters' } })
    // @ts-expect-error Required grouped child parameters are required as static parameters.
    batchRelation({ ...relationConfiguration, parameters: {} })
    // @ts-expect-error Child static grouped values retain their scalar element type.
    batchRelation({ ...relationConfiguration, parameters: { filters: [{ key: 'kind', values: [false] }] } })
  }
  t.is(typeof invalidTypes, 'function')
})
