import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  allGroups,
  and,
  asc,
  constraint,
  count,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  eq,
  exists,
  inParam,
  isNull,
  nullable,
  measure,
  optionalGroupsParam,
  optionalParam,
  ordering,
  project,
  reachable,
  relation,
  requiredListParam,
  requiredParam,
  source,
} from '../dsl.js'
import type {
  Expression,
  ExpressionScalarType,
  QueryGraphError,
  QueryOperation,
  ReachableConfiguration,
  ReachableExpression,
  ResultOf,
} from '../dsl.js'

const roots = source('roots', { id: 'int64', code: 'string' })
const linkFields = {
  id: 'int64',
  parentId: nullable('int64'),
  mainId: 'int64',
  relatedId: 'int64',
  dateDelete: nullable('dateTime'),
} as const
const links = source('links', linkFields)
const main = source('main', { id: 'int64', organisationId: 'int64', dateDelete: nullable('dateTime') })
const related = source('related', { id: 'int64', organisationId: 'int64', dateDelete: nullable('dateTime') })
const organisationId = requiredParam('organisationId', 'int64')
const relatedId = requiredParam('relatedId', 'int64')
const code = requiredParam('code', 'string')
const localModule = defineGraphModule({
  name: 'visibleLinks',
  sources: [links, main, related],
  parameters: [organisationId],
  relations: [
    relation({ name: 'main', from: links, to: main, on: eq(links.field('mainId'), main.field('id')) }),
    relation({ name: 'related', from: links, to: related, on: eq(links.field('relatedId'), related.field('id')) }),
  ],
  constraints: [
    constraint({ predicate: isNull(links.field('dateDelete')) }),
    constraint({
      predicate: exists(main, and(isNull(main.field('dateDelete')), eq(main.field('organisationId'), organisationId))),
    }),
    constraint({
      predicate: exists(
        related,
        and(isNull(related.field('dateDelete')), eq(related.field('organisationId'), organisationId)),
      ),
    }),
  ],
})
const base = {
  name: 'relatedRoots',
  root: roots,
  sources: [roots],
  parameters: [organisationId, relatedId, code],
  projection: [project({ path: 'id', expression: roots.field('id'), default: true })],
  orderings: [ordering({ name: 'id', by: [asc(roots.field('id'))], default: true })],
}
const configuration: ReachableConfiguration<typeof links.key, typeof linkFields> = {
  source: links,
  modules: [localModule],
  key: links.field('id'),
  start: eq(links.field('relatedId'), relatedId),
  step: ({ current, next }) =>
    and(eq(current.field('parentId'), next.field('id')), eq(current.field('mainId'), next.field('relatedId'))),
  match: and(isNull(links.field('parentId')), eq(links.field('mainId'), roots.field('id'))),
}
const mapping = {
  sources: {
    roots: { table: 'Node' },
    links: {
      table: { name: 'Link', schema: 'app', databaseLink: 'remote.example' },
      columns: { id: 'link_id', parentId: 'parent_id' },
    },
    main: { table: 'Node' },
    related: { table: 'Node' },
  },
}
const parameters = { organisationId: 7, relatedId: 20, code: 'catalog' }

function definition(predicate: Expression = reachable(configuration)) {
  return defineGraph({
    ...base,
    constraints: [constraint({ predicate }), constraint({ predicate: eq(roots.field('code'), code) })],
  })
}

test('reachable compiles visibility before traversal and correlates reached rows without root joins', (t) => {
  const graph = registerDefinition(definition()).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters, offset: 10, limit: 5 }, { version: '19c' })
  t.true(statement.sql.includes('FROM "app"."Link"@REMOTE.EXAMPLE "r0_t0"'))
  t.true(statement.sql.includes('"r0_t0"."dateDelete" IS NULL'))
  t.true(statement.sql.includes('"r0_t1"."organisationId" = :p0'))
  t.true(statement.sql.includes('"r0_t2"."organisationId" = :p0'))
  t.true(statement.sql.indexOf('"r0_t2"."dateDelete" IS NULL') < statement.sql.indexOf('START WITH'))
  t.true(statement.sql.includes('START WITH ("r0_walk"."relatedId" = :p1)'))
  t.true(
    statement.sql.includes(
      'CONNECT BY NOCYCLE ((PRIOR "r0_walk"."parent_id" = "r0_walk"."link_id") AND (PRIOR "r0_walk"."mainId" = "r0_walk"."relatedId")) AND PRIOR "r0_walk"."link_id" IS NOT NULL',
    ),
  )
  t.true(statement.sql.includes('("r0_match"."parent_id" IS NULL) AND ("r0_match"."mainId" = "t0"."id")'))
  t.true(statement.sql.endsWith('OFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY'))
  t.false(statement.sql.includes('JOIN'))
  t.false(statement.sql.includes('DISTINCT'))
  t.false(statement.sql.includes('CONNECT_BY_ISLEAF'))
  t.deepEqual(statement.relations, [])
  t.deepEqual(statement.bindings, [
    { name: 'p0', parameter: 'organisationId', scalarType: 'int64' },
    { name: 'p1', parameter: 'relatedId', scalarType: 'int64' },
    { name: 'p2', parameter: 'code', scalarType: 'string' },
  ])
  t.deepEqual(statement.columns, [{ name: 'c0', path: 'id', scalarType: 'int64', nullable: false, relations: [] }])
  t.true(graph.compileOracle({ parameters }, { version: '11g' }).sql.includes('CONNECT BY NOCYCLE'))
})

test('reachable builds its template once and shares no operation-specific state', async (t) => {
  let calls = 0
  const predicate = reachable({
    ...configuration,
    step: (rows) => {
      calls++
      return configuration.step(rows)
    },
  })
  const input = definition(predicate)
  const wire = JSON.stringify(input)
  const graph = registerDefinition(input).withRelationalMapping(mapping)
  const operations = [20, 30, 40].map((id) => ({ parameters: { ...parameters, relatedId: id } }))
  const results = await Promise.all(operations.map(async (operation) => graph.compileOracle(operation)))
  for (const result of results) t.deepEqual(result, results[0])
  t.deepEqual(graph.compileOracle(operations[0]), results[0])
  t.is(calls, 1)
  t.is(JSON.stringify(input), wire)
  t.true(Object.isFrozen(predicate.input))
  t.true(Object.isFrozen(input.constraints[0].predicate))
})

test('reachable keeps independent aliases while sharing parameter bindings', (t) => {
  const predicate = reachable(configuration)
  const graph = registerDefinition(
    definition(and(predicate, reachable(configuration), predicate)),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters })
  for (const index of [0, 1, 2]) t.true(statement.sql.includes(`"r${index}_match"."mainId" = "t0"."id"`))
  t.is(statement.bindings.length, 3)
})

test('reachable preserves scalar, list and grouped binding addresses', (t) => {
  const ids = requiredListParam('ids', 'int64')
  const groups = optionalGroupsParam('groups', { key: 'int64', value: 'int64' })
  const predicate = reachable({
    ...configuration,
    where: and(
      inParam(links.field('id'), ids),
      allGroups(groups, ({ key, values }) =>
        and(eq(links.field('mainId'), key), inParam(links.field('relatedId'), values)),
      ),
    ),
  })
  const input = defineGraph({
    ...base,
    parameters: [...base.parameters, ids, groups],
    constraints: [constraint({ predicate })],
  })
  const graph = registerDefinition(input).withRelationalMapping(mapping)
  const statement = graph.compileOracle({
    parameters: { ...parameters, ids: [1, 2], groups: [{ key: 5, values: [8, 9] }] },
  })
  t.deepEqual(
    statement.bindings.filter(({ parameter }) => parameter === 'ids').map(({ index }) => index),
    [0, 1],
  )
  t.deepEqual(
    statement.bindings.filter(({ parameter }) => parameter === 'groups').map(({ path }) => path),
    [
      [0, 'key'],
      [0, 'values', 0],
      [0, 'values', 1],
    ],
  )
  t.true(statement.bindings.filter(({ parameter }) => parameter === 'groups').every(({ index }) => index === undefined))
  t.is(graph.compileOracle({ parameters: { ...parameters, ids: [] } }).bindings.length, 2)
})

test('reachable plans parameters used only in local relations and start predicates', (t) => {
  const optional = optionalParam('local', 'int64')
  const local = source('local', { id: 'int64' })
  const module = defineGraphModule({
    name: 'localRelation',
    sources: [links, local],
    parameters: [optional],
    relations: [
      relation({
        name: 'local',
        from: links,
        to: local,
        on: and(eq(links.field('mainId'), local.field('id')), eq(local.field('id'), optional)),
      }),
    ],
  })
  const predicate = reachable({ ...configuration, modules: [module], start: exists(local), where: undefined })
  const graph = registerDefinition(
    defineGraph({ ...base, parameters: [...base.parameters, optional], constraints: [constraint({ predicate })] }),
  ).withRelationalMapping({
    sources: { roots: mapping.sources.roots, links: mapping.sources.links, local: { table: 'Node' } },
  })
  const error = t.throws(() => graph.compileOracle({ parameters })) as QueryGraphError
  t.true(error.issues.some(({ code }) => code === 'missingParameter'))
  const statement = graph.compileOracle({ parameters: { ...parameters, local: 4 } })
  t.true(statement.sql.includes('"r0_walk"."mainId" = "r0_t1"."id"'))
  t.deepEqual(
    statement.bindings.map(({ parameter }) => parameter),
    ['local'],
  )
})

test('SQL Server explicitly rejects active reachable predicates but permits inactive constraints', (t) => {
  const enabled = optionalParam('enabled', 'boolean')
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [...base.parameters, enabled],
      constraints: [constraint({ when: enabled, predicate: reachable(configuration) })],
    }),
  ).withRelationalMapping({ sources: { ...mapping.sources, links: { table: 'Link' } } })
  t.false(graph.compileSqlServer({ parameters }).sql.includes('EXISTS'))
  const error = t.throws(() =>
    graph.compileSqlServer({ parameters: { ...parameters, enabled: true } }),
  ) as QueryGraphError
  t.true(error.issues.some(({ code }) => code === 'unsupportedDialectFeature'))
  t.regex(error.message, /reachable traversal/)
  t.true(graph.compileOracle({ parameters: { ...parameters, enabled: true } }).sql.includes('CONNECT BY NOCYCLE'))
})

test('reachable rejects shallow configuration mistakes and projected modules', (t) => {
  for (const config of [
    { ...configuration, typo: true },
    { ...configuration, cycles: 'ignore' },
    { ...configuration, source: null },
    { ...configuration, key: roots.field('id') },
    { ...configuration, modules: {} },
    { ...configuration, start: undefined },
    { ...configuration, match: undefined },
    { ...configuration, step: true },
    { ...configuration, step: () => Promise.resolve(true) },
    {
      ...configuration,
      modules: [
        defineGraphModule({ name: 'projected', projection: [project({ path: 'id', expression: links.field('id') })] }),
      ],
    },
  ])
    t.throws(() => reachable(config as never), { instanceOf: TypeError })
})

test('reachable rejects escaped fields, invalid transitions and incompatible field types', (t) => {
  let escaped: Expression = roots.field('id')
  const predicate = reachable({
    ...configuration,
    step: ({ current, next }) => {
      escaped = current.field('id')
      return eq(current.field('parentId'), next.field('id'))
    },
  })
  const cases: Array<[Expression, string]> = [
    [eq(escaped, roots.field('id')), 'invalidTraversalField'],
    [
      reachable({ ...configuration, step: ({ current }) => eq(current.field('id'), current.field('parentId')) }),
      'invalidReachableDefinition',
    ],
    [
      reachable({ ...configuration, step: ({ current, next }) => eq(current.field('id'), next.field('dateDelete')) }),
      'incompatibleExpressionTypes',
    ],
    [reachable({ ...configuration, step: () => true }), 'invalidReachableDefinition'],
    [{ ...predicate, match: escaped }, 'invalidTraversalField'],
    [reachable({ ...configuration, key: links.field('parentId') }), 'invalidReachableDefinition'],
    [reachable({ ...configuration, start: links.field('id') }), 'invalidPredicateType'],
    [reachable({ ...configuration, match: links.field('id') }), 'invalidPredicateType'],
  ]
  for (const [invalid, expectedCode] of cases) {
    const error = t.throws(() => registerDefinition(definition(invalid))) as QueryGraphError
    t.true(
      error.issues.some(({ code }) => code === expectedCode),
      JSON.stringify(error.issues),
    )
  }
})

test('reachable validates lexical boundaries and local module contracts', (t) => {
  const predicate = reachable(configuration)
  const cases: Expression[] = [
    reachable({ ...configuration, where: eq(roots.field('id'), 1) }),
    reachable({ ...configuration, where: eq(main.field('id'), 1) }),
    reachable({ ...configuration, start: eq(roots.field('id'), 1) }),
    reachable({ ...configuration, where: predicate }),
    { ...predicate, input: { ...predicate.input, sources: [...predicate.input.sources, roots] } },
    { ...predicate, input: { ...predicate.input, parameters: [requiredParam('missing', 'int64')] } },
    { ...predicate, input: { ...predicate.input, parameters: [requiredParam('organisationId', 'string')] } },
    { ...predicate, input: { ...predicate.input, parameters: [organisationId, organisationId] } },
    reachable({ ...configuration, match: eq(count(), 1) }),
    { ...predicate, scope: '' },
    { ...predicate, step: eq(roots.field('id'), 1) },
  ]
  for (const invalid of cases) {
    const error = t.throws(() => registerDefinition(definition(invalid))) as QueryGraphError
    t.true(
      error.issues.some(({ code }) => code === 'invalidReachableDefinition'),
      JSON.stringify(error.issues),
    )
  }
  const error = t.throws(() => registerDefinition(definition(eq(predicate, true)))) as QueryGraphError
  t.true(error.issues.some(({ code }) => code === 'invalidReachableContext'))
})

test('reachable separates pre-aggregation filters from aggregate constraints', (t) => {
  const predicate = reachable(configuration)
  const summary = {
    name: 'rootCount',
    root: roots,
    sources: [roots],
    parameters: base.parameters,
    dimensions: [],
    measures: [measure({ path: 'count', expression: count(), default: true })],
  }
  const graph = registerDefinition(
    defineSummaryGraph({ ...summary, constraints: [constraint({ predicate })] }),
  ).withRelationalMapping(mapping)
  t.true(graph.compileOracle({ parameters }).sql.includes('WHERE\n  EXISTS'))
  const error = t.throws(() =>
    registerDefinition(
      defineSummaryGraph({
        ...summary,
        constraints: [constraint({ predicate: and(predicate, eq(count(), 1)) })],
      }),
    ),
  ) as QueryGraphError
  t.true(error.issues.some(({ code }) => code === 'invalidReachableContext'))
})

test('reachable validates scopes and shared local mapping keys across predicates', (t) => {
  const first = reachable(configuration)
  const second = reachable(configuration)
  const differentFields = {
    ...second,
    input: {
      ...second.input,
      sources: second.input.sources.map((item) => ({
        ...item,
        fields: [...item.fields, { name: 'extra', scalarType: 'string' as const }],
      })),
    },
  }
  for (const invalid of [
    { ...second, step: first.step },
    {
      ...second,
      scope: first.scope,
      step: first.step,
      match: first.match,
      input: { ...second.input, name: 'different' },
    },
    differentFields,
  ]) {
    const error = t.throws(() => registerDefinition(definition(and(first, invalid)))) as QueryGraphError
    t.true(error.issues.some(({ code }) => code === 'invalidReachableDefinition'))
  }
  const graph = registerDefinition(definition(first))
  const error = t.throws(() =>
    graph.withRelationalMapping({ sources: { roots: mapping.sources.roots } }),
  ) as QueryGraphError
  t.is(error.phase, 'mapping')
  t.true(error.issues.some(({ location }) => location === 'sources.links'))
})

test('reachable honors conditional module constraints without requiring inactive parameters', (t) => {
  const enabled = optionalParam('enabled', 'boolean')
  const id = optionalParam('id', 'int64')
  const module = defineGraphModule({
    name: 'conditional',
    parameters: [enabled, id],
    constraints: [constraint({ when: enabled, predicate: eq(links.field('id'), id) })],
  })
  const graph = registerDefinition(
    defineGraph({
      ...base,
      parameters: [...base.parameters, enabled, id],
      constraints: [constraint({ predicate: reachable({ ...configuration, modules: [module] }) })],
    }),
  ).withRelationalMapping({ sources: { roots: mapping.sources.roots, links: mapping.sources.links } })
  t.deepEqual(
    graph.compileOracle({ parameters }).bindings.map(({ parameter }) => parameter),
    ['relatedId'],
  )
  const error = t.throws(() =>
    graph.compileOracle({ parameters: { ...parameters, enabled: false } }),
  ) as QueryGraphError
  t.true(error.issues.some(({ code }) => code === 'missingParameter'))
  t.deepEqual(
    graph
      .compileOracle({ parameters: { ...parameters, enabled: true, id: 2 } })
      .bindings.map(({ parameter }) => parameter),
    ['id', 'relatedId'],
  )
})

test('reachable plans outer match dependencies without leaking local sources into joins', (t) => {
  const detail = source('detail', { id: 'int64' })
  const graph = registerDefinition(
    defineGraph({
      ...base,
      sources: [roots, detail],
      relations: [relation({ name: 'detail', from: roots, to: detail, on: eq(roots.field('id'), detail.field('id')) })],
      constraints: [
        constraint({
          predicate: reachable({ ...configuration, match: eq(links.field('mainId'), detail.field('id')) }),
        }),
      ],
    }),
  ).withRelationalMapping({ sources: { ...mapping.sources, detail: { table: 'Detail' } } })
  const statement = graph.compileOracle({ parameters })
  t.deepEqual(
    statement.relations.map(({ name }) => name),
    ['detail'],
  )
  t.true(statement.sql.includes('"r0_match"."mainId" = "t1"."id"'))
  t.is(statement.sql.match(/LEFT JOIN/g)?.length, 1)
})

test('reachable rejects unknown wire fields and preserves strict serialization', (t) => {
  const predicate = reachable(configuration)
  const error = t.throws(() =>
    registerDefinition(definition({ ...predicate, typo: true } as ReachableExpression)),
  ) as QueryGraphError
  t.is(error.phase, 'definition')
  const serialized = JSON.parse(JSON.stringify(definition(predicate)))
  t.notThrows(() => registerDefinition(serialized))
})

test('reachable keeps operation and result inference while typing step fields', (t) => {
  const predicate = reachable({
    ...configuration,
    step: ({ current, next }) => {
      const type: ExpressionScalarType<ReturnType<typeof current.field<'id'>>> = 'int64'
      t.is(type, 'int64')
      return eq(current.field('parentId'), next.field('id'))
    },
  })
  const input = definition(predicate)
  const operation: QueryOperation<typeof input> = { parameters, select: ['id'] }
  const row: ResultOf<typeof input> = { id: 1 }
  t.is(row.id, 1)
  t.deepEqual(operation.select, ['id'])
  const typeErrors = () => {
    reachable({
      ...configuration,
      step: ({ next }) => {
        // @ts-expect-error Traversal fields come from the local source.
        return eq(next.field('missing'), 1)
      },
    })
    // @ts-expect-error The row key must belong to the traversal source.
    reachable({ ...configuration, key: roots.field('id') })
    // @ts-expect-error Parameters remain inferred from the outer definition.
    const invalid: QueryOperation<typeof input> = { parameters: { organisationId: true, relatedId: 1, code: 'root' } }
    void invalid
  }
  void typeErrors
})
