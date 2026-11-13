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
  dimension,
  eq,
  exists,
  firstBy,
  hidden,
  inParam,
  isNull,
  measure,
  not,
  nullable,
  optionalGroupsParam,
  optionalListParam,
  optionalParam,
  or,
  ordering,
  pathEquals,
  project,
  projectObject,
  reachable,
  relation,
  requiredListParam,
  source,
  traversalSource,
} from '../dsl.js'
import type { Expression, QueryGraphError, QueryOperation, ResultOf, TraversalRow } from '../dsl.js'

const nodeFields = {
  id: 'int64',
  parentId: nullable('int64'),
  ownerId: 'int64',
  targetId: 'int64',
  organisationId: 'int64',
  name: 'string',
  deletedAt: nullable('dateTime'),
  secret: hidden('string'),
} as const
const nodes = source('nodes', nodeFields)
const owners = source('owners', { id: 'int64' })
const organisationId = optionalParam('organisationId', 'int64')
const path = optionalListParam('path', 'int64')
const configuration = {
  name: 'tree',
  source: nodes,
  key: nodes.field('id'),
  where: and(isNull(nodes.field('deletedAt')), eq(nodes.field('organisationId'), organisationId)),
  start: isNull(nodes.field('parentId')),
  step: ({ current, next }: { current: TraversalRow<typeof nodeFields>; next: TraversalRow<typeof nodeFields> }) =>
    eq(current.field('id'), next.field('parentId')),
  path: { prefix: nodes.field('ownerId'), node: nodes.field('targetId') },
} as const
const tree = traversalSource(configuration)
const parameters = [organisationId, path]
const mapping = {
  sources: {
    nodes: {
      table: { name: 'Nodes', schema: 'app', databaseLink: 'remote.example' },
      columns: { id: 'node_id', parentId: 'parent_id' },
    },
  },
}
const operation = { parameters: { organisationId: 7 } }
const rootConfiguration = {
  name: 'nodeTree',
  root: tree,
  sources: [tree],
  parameters,
  projection: [
    project({ path: 'id', expression: tree.field('id'), default: true }),
    project({ path: 'rootId', expression: tree.rootField('id'), default: true }),
    project({ path: 'level', expression: tree.level(), default: true }),
    project({ path: 'name', expression: tree.field('name') }),
  ],
  orderings: [ordering({ name: 'level', by: [asc(tree.level()), asc(tree.field('id'))], default: true })],
}
const rootDefinition = defineGraph(rootConfiguration)
const rootGraph = registerDefinition(rootDefinition).withRelationalMapping(mapping)
const treeRelation = relation({
  name: 'tree',
  from: owners,
  to: tree,
  cardinality: 'many',
  on: eq(owners.field('id'), tree.rootField('ownerId')),
})
const joinedConfiguration = {
  name: 'owners',
  root: owners,
  sources: [owners, tree],
  parameters,
  relations: [treeRelation],
  projection: [
    project({ path: 'id', expression: owners.field('id'), default: true }),
    project({ path: 'tree.id', expression: tree.field('id') }),
    project({ path: 'tree.rootId', expression: tree.rootField('id') }),
    project({ path: 'tree.level', expression: tree.level() }),
  ],
  objects: [projectObject({ path: 'tree', presence: tree.field('id') })],
}
const joinedMapping = { sources: { ...mapping.sources, owners: { table: 'Owners' } } }

test('traversal source compiles as root with typed fields and metadata', (t) => {
  const statement = rootGraph.compileOracle({ ...operation, limit: 10 }, { version: '19c' })
  t.true(statement.sql.includes('"t0"."qg_field_0" AS "c0"'))
  t.true(statement.sql.includes('"app"."Nodes"@REMOTE.EXAMPLE'))
  t.true(statement.sql.includes('CONNECT_BY_ROOT "r0_walk"."node_id" AS "qg_root_0"'))
  t.true(statement.sql.includes('LEVEL AS "qg_level"'))
  t.true(statement.sql.indexOf('"r0_t0"."deletedAt" IS NULL') < statement.sql.indexOf('START WITH'))
  t.true(statement.sql.includes('PRIOR "r0_walk"."node_id" = "r0_walk"."parent_id"'))
  t.true(statement.sql.includes('AND PRIOR "r0_walk"."node_id" IS NOT NULL'))
  t.false(statement.sql.includes('DISTINCT'))
  t.deepEqual(
    statement.columns.map(({ path, scalarType, nullable }) => ({ path, scalarType, nullable })),
    [
      { path: 'id', scalarType: 'int64', nullable: false },
      { path: 'rootId', scalarType: 'int64', nullable: false },
      { path: 'level', scalarType: 'int32', nullable: false },
    ],
  )
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'organisationId', scalarType: 'int64' }])
  t.true(rootGraph.compileOracle(operation, { version: '11g' }).sql.includes('CONNECT BY NOCYCLE'))
})

test('traversal source joins are lazy and preserve optional projection presence', (t) => {
  const graph = registerDefinition(defineGraph(joinedConfiguration)).withRelationalMapping(joinedMapping)
  t.false(graph.compileSqlServer({ select: ['id'] }).sql.includes('JOIN'))
  const statement = graph.compileOracle({ ...operation, select: ['id', 'tree.rootId', 'tree.level'] })
  t.true(statement.sql.includes('LEFT JOIN ('))
  t.true(statement.sql.includes('"t0"."id" = "t1"."qg_root_2"'))
  t.true(statement.columns.slice(1).every(({ nullable, relations }) => nullable && relations[0] === 'tree'))
  t.deepEqual(statement.objects, [{ path: 'tree', presenceColumn: 'o0' }])
  const missing = t.throws(() => graph.compileOracle({ select: ['tree.level'] })) as QueryGraphError
  t.true(missing.issues.some(({ code }) => code === 'missingParameter'))
  const unsupported = t.throws(() =>
    graph.compileSqlServer({ ...operation, select: ['tree.level'] }),
  ) as QueryGraphError
  t.true(unsupported.issues.some(({ code }) => code === 'unsupportedDialectFeature'))
})

test('traversal source shares its physical input with an outer source without duplicating mappings', (t) => {
  const configuration = {
    name: 'nodesWithTree',
    root: nodes,
    sources: [nodes, tree],
    parameters,
    relations: [
      relation({
        name: 'descendants',
        from: nodes,
        to: tree,
        cardinality: 'many',
        on: eq(nodes.field('id'), tree.rootField('id')),
      }),
    ],
    projection: [
      project({ path: 'id', expression: nodes.field('id'), default: true }),
      project({ path: 'descendant.id', expression: tree.field('id') }),
    ],
  }
  const graph = registerDefinition(defineGraph(configuration)).withRelationalMapping(mapping)
  const sql = graph.compileOracle({ ...operation, select: ['id', 'descendant.id'] }).sql
  t.true(sql.includes('"t0"."node_id" = "t1"."qg_root_0"'))
  t.is(sql.split('"app"."Nodes"@REMOTE.EXAMPLE').length - 1, 2)
  const invalid = { ...nodes, fields: nodes.fields.slice(1) }
  const error = t.throws(() =>
    registerDefinition(defineGraph({ ...configuration, sources: [invalid, tree] })),
  ) as QueryGraphError
  t.true(
    error.issues.some(
      ({ code, location }) =>
        code === 'invalidTraversalDefinition' && location === 'sources[1].traversal.input.sources',
    ),
  )
})

test('traversal source supports outgoing joins, exists and firstBy', (t) => {
  const details = source('details', { id: 'int64', name: 'string' })
  const graph = registerDefinition(
    defineGraph({
      ...rootConfiguration,
      sources: [tree, details],
      relations: [
        relation({ name: 'detail', from: tree, to: details, on: eq(tree.field('targetId'), details.field('id')) }),
      ],
      projection: [project({ path: 'name', expression: details.field('name'), default: true })],
    }),
  ).withRelationalMapping({ sources: { ...mapping.sources, details: { table: 'Details' } } })
  t.true(graph.compileOracle(operation).sql.includes('"t0"."qg_field_3" = "t1"."id"'))

  const selection = firstBy(asc(tree.level()), asc(tree.field('id')))
  const one = relation({ name: 'tree', from: owners, to: tree, selection, on: treeRelation.on })
  for (const relations of [[treeRelation], [one]]) {
    const graph = registerDefinition(
      defineGraph({ ...joinedConfiguration, relations, constraints: [constraint({ predicate: exists(tree) })] }),
    ).withRelationalMapping(joinedMapping)
    const statement = graph.compileOracle(operation)
    t.true(statement.sql.includes('EXISTS'))
    t.true(statement.sql.includes('CONNECT BY NOCYCLE'))
    t.deepEqual(statement.relations, [])
    t.throws(() => graph.compileOracle({}))
  }
  const selected = registerDefinition(defineGraph({ ...joinedConfiguration, relations: [one] })).withRelationalMapping(
    joinedMapping,
  )
  t.true(selected.compileOracle({ ...operation, select: ['tree.level'] }).sql.includes('OUTER APPLY'))
})

test('pathEquals preserves positions, repetition and indexed bindings without string serialization', (t) => {
  const definition = defineGraph({
    ...rootConfiguration,
    constraints: [constraint({ when: path, predicate: pathEquals(tree.path(), path) })],
  })
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters: { organisationId: 7, path: [10, 20, 10, 30] } })
  t.true(statement.sql.includes('"r0_walk"."ownerId" = :p1'))
  t.true(statement.sql.includes('"r0_walk"."targetId" = :p2'))
  t.true(statement.sql.includes('CASE LEVEL WHEN 2 THEN :p3 WHEN 3 THEN :p4 END'))
  t.true(statement.sql.includes('"t0"."qg_level" = 3'))
  t.false(statement.sql.includes('SYS_CONNECT_BY_PATH'))
  t.deepEqual(
    statement.bindings.filter(({ parameter }) => parameter === 'path').map(({ index }) => index),
    [0, 1, 2, 3],
  )
  const noPath = graph.compileOracle(operation)
  t.false(noPath.sql.includes('LEVEL <='))
  for (const values of [[], [10]]) {
    const empty = graph.compileOracle({ parameters: { organisationId: 7, path: values } })
    t.true(empty.sql.includes('(1 = 0)'))
    t.true(empty.sql.includes('"qg_level" = 0'))
  }
  const direct = graph.compileOracle({ parameters: { organisationId: 7, path: [10, 20] } })
  t.true(direct.sql.includes('"qg_level" = 1'))
  for (const invalid of [null, 1, [null], ['invalid'], { values: [1] }]) {
    t.throws(() => graph.compileOracle({ parameters: { organisationId: 7, path: invalid } } as never))
  }
})

test('pathEquals does not restrict independent EXISTS occurrences of the same source', (t) => {
  const graph = registerDefinition(
    defineGraph({
      ...joinedConfiguration,
      constraints: [
        constraint({
          when: path,
          predicate: and(pathEquals(tree.path(), path), not(exists(tree, eq(tree.field('targetId'), 999)))),
        }),
      ],
    }),
  ).withRelationalMapping(joinedMapping)
  const sql = graph.compileOracle({ parameters: { organisationId: 7, path: [10, 20, 30] } }).sql
  t.is(sql.split('CONNECT BY NOCYCLE').length - 1, 2)
  t.is(sql.split('CASE LEVEL').length - 1, 1)
})

test('traversal sources compose modules and preserve nested grouped bindings', (t) => {
  const groups = optionalGroupsParam('groups', { key: 'int64', value: 'int64' })
  const module = defineGraphModule({
    name: 'input',
    parameters: [groups],
    constraints: [
      constraint({
        predicate: allGroups(groups, ({ key, values }) =>
          and(eq(nodes.field('ownerId'), key), inParam(nodes.field('targetId'), values)),
        ),
      }),
    ],
  })
  const filtered = traversalSource({ ...configuration, modules: [module] })
  const feature = defineGraphModule({
    name: 'tree',
    sources: [filtered],
    projection: [project({ path: 'id', expression: filtered.field('id'), default: true })],
  })
  const graph = registerDefinition(
    defineGraph({ name: 'composed', root: filtered, modules: [feature], parameters: [...parameters, groups] }),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle({ parameters: { organisationId: 7, groups: [{ key: 10, values: [20, 30] }] } })
  t.deepEqual(
    statement.bindings.filter(({ parameter }) => parameter === 'groups').map(({ path }) => path),
    [
      [0, 'key'],
      [0, 'values', 0],
      [0, 'values', 1],
    ],
  )
})

test('traversal source composes with reachable and keeps independent scopes', (t) => {
  const predicate = reachable({
    source: nodes,
    key: nodes.field('id'),
    start: configuration.start,
    step: configuration.step,
    where: configuration.where,
    match: eq(nodes.field('id'), tree.field('id')),
  })
  const graph = registerDefinition(
    defineGraph({ ...rootConfiguration, constraints: [constraint({ predicate })] }),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle(operation)
  t.is(statement.sql.split('CONNECT BY NOCYCLE').length - 1, 2)
  t.true(statement.sql.includes('"r1_match"."node_id" = "t0"."qg_field_0"'))
  t.is(statement.bindings.length, 1)
})

test('traversal source supports summary grouping by root metadata', (t) => {
  const graph = registerDefinition(
    defineSummaryGraph({
      name: 'counts',
      root: tree,
      sources: [tree],
      parameters,
      dimensions: [dimension({ path: 'rootId', expression: tree.rootField('id'), default: true })],
      measures: [measure({ path: 'count', expression: count(), default: true })],
    }),
  ).withRelationalMapping(mapping)
  t.true(graph.compileOracle(operation).sql.includes('GROUP BY\n  "t0"."qg_root_0"'))
  t.throws(() =>
    registerDefinition(
      defineSummaryGraph({
        name: 'invalid',
        root: tree,
        sources: [tree],
        parameters,
        dimensions: [],
        measures: [measure({ path: 'count', expression: count(), default: true })],
        constraints: [constraint({ predicate: eq(count(), tree.level()) })],
      }),
    ),
  )
})

test('traversal sources participate in deferred batch plans', (t) => {
  const ownerIds = requiredListParam('ownerIds', 'int64')
  const child = registerDefinition(
    defineGraph({
      ...rootConfiguration,
      parameters: [...parameters, ownerIds],
      constraints: [constraint({ predicate: inParam(tree.rootField('ownerId'), ownerIds) })],
      projection: [
        project({ path: 'id', expression: tree.field('id') }),
        project({ path: 'level', expression: tree.level() }),
        project({ path: 'ownerId', expression: tree.rootField('ownerId') }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const parent = registerDefinition(
    defineGraph({
      name: 'owners',
      root: owners,
      sources: [owners],
      projection: [project({ path: 'id', expression: owners.field('id'), default: true })],
    }),
  ).withRelationalMapping({ sources: { owners: { table: 'Owners' } } })
  const query = batchQuery({ graph: child, key: { path: 'ownerId', parameter: ownerIds } })
  const graph = composeGraph({
    root: parent,
    relations: [
      batchRelation({
        name: 'nodes',
        from: 'id',
        cardinality: 'many',
        parameters: { organisationId: 7 },
        query,
      }),
    ],
  })
  const plan = graph.compileOraclePlan({ select: ['id', 'nodes.id', 'nodes.level'] })
  t.false(plan.root.sql.includes('CONNECT BY'))
  t.is(plan.batches[0].cardinality, 'many')
  const statement = plan.compileBatch('nodes', [1, 2])
  t.true(statement.sql.includes('CONNECT BY NOCYCLE'))
  t.true(statement.sql.includes('"t0"."qg_root_2" IN (:p1, :p2)'))
  t.deepEqual(
    statement.bindings.filter(({ parameter }) => parameter === 'ownerIds').map(({ index }) => index),
    [0, 1],
  )
})

test('traversal paths support strings and multiple exact constraints without losing positions', (t) => {
  const stringPath = optionalListParam('stringPath', 'string')
  const alternate = optionalListParam('alternatePath', 'string')
  const named = traversalSource({ ...configuration, path: { node: nodes.field('name') } })
  const graph = registerDefinition(
    defineGraph({
      ...rootConfiguration,
      sources: [named],
      parameters: [...parameters, stringPath, alternate],
      constraints: [
        constraint({ predicate: and(pathEquals(named.path(), stringPath), pathEquals(named.path(), alternate)) }),
      ],
    }),
  ).withRelationalMapping(mapping)
  const statement = graph.compileOracle({
    parameters: { organisationId: 7, stringPath: ['a/b', 'x%y'], alternatePath: ['a/b', 'x%y'] },
  })
  t.true(statement.sql.includes('"r0_walk"."name" = :p1'))
  t.true(statement.sql.includes('"r0_walk"."name" = :p3'))
  t.false(statement.sql.includes('a/b'))
  t.false(statement.sql.includes('x%y'))
  t.deepEqual(
    statement.bindings
      .filter(({ scalarType }) => scalarType === 'string')
      .map(({ parameter, index }) => [parameter, index]),
    [
      ['alternatePath', 0],
      ['alternatePath', 1],
      ['stringPath', 0],
      ['stringPath', 1],
    ],
  )
})

test('traversal source rejects invalid configuration, lexical scopes and forged schemas', (t) => {
  for (const invalid of [
    { ...configuration, typo: true },
    { ...configuration, cycles: 'stop' },
    { ...configuration, name: '' },
    { ...configuration, key: owners.field('id') },
    { ...configuration, path: { node: owners.field('id') } },
    { ...configuration, path: { node: nodes.field('id'), separator: '/' } },
    { ...configuration, source: tree },
  ])
    t.throws(() => traversalSource(invalid as never), { instanceOf: TypeError })
  for (const invalid of [
    traversalSource({ ...configuration, where: eq(owners.field('id'), 1) }),
    traversalSource({ ...configuration, key: nodes.field('parentId') }),
    traversalSource({ ...configuration, step: ({ current }) => eq(current.field('id'), current.field('parentId')) }),
    traversalSource({ ...configuration, path: { node: nodes.field('deletedAt') } }),
    traversalSource({ ...configuration, path: { node: nodes.field('name'), prefix: nodes.field('id') } }),
    { ...tree, fields: tree.fields.slice(1) },
  ])
    t.throws(() => registerDefinition(defineGraph({ ...rootConfiguration, sources: [invalid as never] })))
  t.throws(() =>
    registerDefinition(
      defineGraph({
        ...rootConfiguration,
        projection: [project({ path: 'secret', expression: tree.rootField('secret') })],
      }),
    ),
  )
  const noPath = traversalSource({ ...configuration, path: undefined })
  t.throws(() => (noPath.path as () => unknown)(), { instanceOf: TypeError })
  let escaped: Expression = nodes.field('id')
  const scoped = traversalSource({
    ...configuration,
    step: ({ current, next }) => {
      escaped = current.field('id')
      return eq(escaped, next.field('parentId'))
    },
  })
  t.throws(() =>
    registerDefinition(
      defineGraph({
        ...rootConfiguration,
        sources: [scoped],
        constraints: [constraint({ predicate: eq(escaped, 1) })],
      }),
    ),
  )
})

test('pathEquals rejects contexts where pushdown would change semantics', (t) => {
  for (const predicate of [
    not(pathEquals(tree.path(), path)),
    or(pathEquals(tree.path(), path), eq(tree.field('id'), 1)),
    eq(pathEquals(tree.path(), path), true),
  ]) {
    const error = t.throws(() =>
      registerDefinition(defineGraph({ ...rootConfiguration, constraints: [constraint({ predicate })] })),
    ) as QueryGraphError
    t.true(error.issues.some(({ code }) => code === 'invalidTraversalContext'))
  }
  const one = relation({
    name: 'tree',
    from: owners,
    to: tree,
    on: treeRelation.on,
    selection: firstBy(asc(tree.field('id'))),
  })
  t.throws(() =>
    registerDefinition(
      defineGraph({
        ...joinedConfiguration,
        relations: [one],
        constraints: [constraint({ predicate: pathEquals(tree.path(), path) })],
      }),
    ),
  )
  t.throws(() =>
    registerDefinition(
      defineGraph({
        ...rootConfiguration,
        projection: [project({ path: 'match', expression: pathEquals(tree.path(), path) })],
      }),
    ),
  )
})

test('traversal sources require only physical mappings and validate nested wire fields', (t) => {
  const graph = registerDefinition(rootDefinition)
  t.throws(() => graph.withRelationalMapping({ sources: {} }))
  t.throws(() => graph.withRelationalMapping({ sources: { ...mapping.sources, tree: { table: 'NotATable' } } }))
  const wire = JSON.parse(JSON.stringify(rootDefinition))
  wire.sources[0].traversal.typo = true
  const error = t.throws(() => registerDefinition(wire)) as QueryGraphError
  t.truthy(error)
})

test('one registered traversal graph handles concurrent path operations without mutation', async (t) => {
  let calls = 0
  const reusable = traversalSource({
    ...configuration,
    step: (rows) => {
      calls++
      return configuration.step(rows as never)
    },
  })
  const definition = defineGraph({
    ...rootConfiguration,
    sources: [reusable],
    constraints: [constraint({ when: path, predicate: pathEquals(reusable.path(), path) })],
  })
  const original = JSON.stringify(definition)
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const statements = await Promise.all(
    [[], [1, 2], [1, 2, 1, 3]].map(async (path) => graph.compileOracle({ parameters: { organisationId: 7, path } })),
  )
  t.deepEqual(
    statements.map(({ bindings }) => bindings.length),
    [1, 3, 5],
  )
  t.is(JSON.stringify(definition), original)
  t.is(calls, 1)
  t.deepEqual(graph.compileOracle({ parameters: { organisationId: 7, path: [1, 2] } }), statements[1])
})

test('traversal source preserves SourceRef, select, parameter and result typing', (t) => {
  type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false
  const result: Equal<
    ResultOf<typeof rootGraph>,
    { id: number | string; rootId: number | string; level: number }
  > = true
  const input: QueryOperation<typeof rootDefinition> = {
    select: ['rootId', 'level'],
    parameters: { organisationId: 7, path: [1, '2', 3] },
  }
  const typeErrors = () => {
    // @ts-expect-error inherited fields retain their literal names
    tree.field('typo')
    // @ts-expect-error a path cannot borrow fields from a different source
    traversalSource({ ...configuration, path: { node: owners.field('id') } })
    // @ts-expect-error root metadata retains its input field names
    tree.rootField('typo')
    // @ts-expect-error path and parameter element types must agree
    pathEquals(tree.path(), optionalListParam('strings', 'string'))
    // @ts-expect-error selected metadata does not become an arbitrary string path
    rootGraph.compileOracle({ select: ['unknown'] })
    const noPath = traversalSource({ ...configuration, path: undefined })
    // @ts-expect-error no path reference exists without a path definition
    noPath.path()
  }
  void typeErrors
  t.true(result)
  t.truthy(rootGraph.compileOracle(input))
})
