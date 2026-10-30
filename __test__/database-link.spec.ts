import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  and,
  asc,
  constraint,
  defineGraph,
  eq,
  exists,
  firstBy,
  optionalParam,
  ordering,
  project,
  projectObject,
  relation,
  source,
} from '../dsl.js'
import type { QueryGraphError, RelationalMapping } from '../dsl.js'

const users = source('users', { id: 'int64' })
const profiles = source('profiles', { id: 'int64', userId: 'int64', name: 'string' })
const flags = source('flags', { profileId: 'int64', active: 'boolean' })
const profileId = optionalParam('profileId', 'int64')
const definition = defineGraph({
  name: 'remoteProfiles',
  root: users,
  sources: [users, profiles, flags],
  parameters: [profileId],
  relations: [
    relation({
      name: 'profile',
      from: users,
      to: profiles,
      on: and(
        eq(users.field('id'), profiles.field('userId')),
        exists(flags, eq(flags.field('active'), true), { from: profiles }),
      ),
      selection: firstBy(asc(profiles.field('id'))),
    }),
    relation({
      name: 'flags',
      from: profiles,
      to: flags,
      on: eq(profiles.field('id'), flags.field('profileId')),
      cardinality: 'many',
    }),
  ],
  constraints: [constraint({ when: profileId, predicate: eq(profiles.field('id'), profileId) })],
  orderings: [ordering({ name: 'idAsc', by: [asc(users.field('id'))] })],
  objects: [projectObject({ path: 'profile', presence: profiles.field('id') })],
  projection: [
    project({ path: 'id', expression: users.field('id'), default: true }),
    project({ path: 'profile.name', expression: profiles.field('name') }),
  ],
})
const mapping: RelationalMapping = {
  sources: {
    users: { table: 'Users' },
    profiles: { table: { schema: 'App', name: 'User#Profile', databaseLink: 'remote_gate' } },
    flags: { table: { name: 'Flags', databaseLink: 'remote.example.com@reader' } },
  },
}

test('renders database links inside firstBy and correlated exists without changing metadata', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const localGraph = registerDefinition(definition).withRelationalMapping({
    sources: {
      users: mapping.sources.users,
      profiles: { table: { schema: 'App', name: 'User#Profile' } },
      flags: { table: 'Flags' },
    },
  })
  const operation = {
    select: ['id', 'profile.name'],
    parameters: { profileId: 42 },
    ordering: 'idAsc',
    limit: 10,
  } as const
  const { sql, ...metadata } = graph.compileOracle(operation, { version: '19c' })
  const { sql: localSql, ...localMetadata } = localGraph.compileOracle(operation, { version: '19c' })

  t.is(
    sql,
    localSql
      .replace('"App"."User#Profile"', '"App"."User#Profile"@REMOTE_GATE')
      .replace('"Flags"', '"Flags"@REMOTE.EXAMPLE.COM@READER'),
  )
  t.true(sql.includes('OUTER APPLY'))
  t.true(sql.includes('EXISTS ('))
  t.deepEqual(metadata, localMetadata)
  t.deepEqual(metadata.bindings, [{ name: 'p0', parameter: 'profileId', scalarType: 'int64' }])
  t.deepEqual(metadata.objects, [{ path: 'profile', presenceColumn: 'o0' }])
})

test('does not add remote relations that are not selected or filtered', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  t.is(graph.compileOracle({}).sql, 'SELECT\n  "t0"."id" AS "c0"\nFROM "Users" "t0"')
  t.is(graph.compileSqlServer({}).sql, 'SELECT\n  [t0].[id] AS [c0]\nFROM [Users] AS [t0]')
})

test('returns an explicit SQL Server diagnostic for a referenced database link', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  for (const operation of [{ select: ['profile.name'] as const }, { parameters: { profileId: 42 } }]) {
    const error = t.throws(() => graph.compileSqlServer(operation)) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_SQL_COMPILE_FAILED')
    t.is(error.phase, 'sql')
    t.like(error.issues[0], { code: 'unsupportedTableQualifier', location: 'mapping' })
    t.regex(error.message, /SQL Server.*databaseLink/)
  }
})

test('returns a precise mapping diagnostic for unsafe or malformed database link names', (t) => {
  const graph = registerDefinition(definition)
  for (const databaseLink of ['', '@REMOTE', 'REMOTE..COM', 'REMOTE;SELECT 1', 'REMOTE--comment', 'A'.repeat(129)]) {
    const error = t.throws(() =>
      graph.withRelationalMapping({
        sources: { ...mapping.sources, users: { table: { name: 'Users', databaseLink } } },
      }),
    ) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_MAPPING_INVALID')
    t.is(error.phase, 'mapping')
    t.is(error.issues.length, 1)
    t.like(error.issues[0], { code: 'invalidDatabaseLink', location: 'sources.users.table.databaseLink' })
  }
})

test('rejects non-string database links in TypeScript and at the wire boundary', (t) => {
  const graph = registerDefinition(definition)
  const error = t.throws(() =>
    graph.withRelationalMapping({
      sources: {
        ...mapping.sources,
        users: {
          table: {
            name: 'Users',
            // @ts-expect-error Database links are physical names, not operation values.
            databaseLink: 42,
          },
        },
      },
    }),
  ) as QueryGraphError
  t.is(error.code, 'QUERY_GRAPH_MAPPING_WIRE_INVALID')
})

test('merges equivalent remote mappings and rejects different destinations', (t) => {
  const graph = registerDefinition(definition)
  const remote = graph.withRelationalMappings([
    mapping,
    { sources: { profiles: { table: { schema: 'App', name: 'User#Profile', databaseLink: 'REMOTE_GATE' } } } },
  ])
  t.deepEqual(
    remote.compileOracle({ select: ['profile.name'] }),
    graph.withRelationalMapping(mapping).compileOracle({ select: ['profile.name'] }),
  )
  for (const table of [
    { schema: 'App', name: 'User#Profile' },
    { schema: 'App', name: 'User#Profile', databaseLink: 'OTHER_GATE' },
  ]) {
    const error = t.throws(() =>
      graph.withRelationalMappings([mapping, { sources: { profiles: { table } } }]),
    ) as QueryGraphError
    t.is(error.code, 'QUERY_GRAPH_MAPPING_INVALID')
    t.like(error.issues[0], { code: 'conflictingSourceTable', location: 'mappings[1].sources.profiles.table' })
  }
})

test('keeps @ inside table names quoted and still rejects catalog in Oracle', (t) => {
  const graph = registerDefinition(definition)
  const literalName = graph.withRelationalMapping({
    sources: { ...mapping.sources, users: { table: 'Users@REMOTE' } },
  })
  t.true(literalName.compileOracle({}).sql.includes('FROM "Users@REMOTE" "t0"'))
  const catalog = graph.withRelationalMapping({
    sources: {
      ...mapping.sources,
      users: { table: { catalog: 'Application', name: 'Users', databaseLink: 'REMOTE' } },
    },
  })
  const error = t.throws(() => catalog.compileOracle({})) as QueryGraphError
  t.like(error.issues[0], { code: 'unsupportedTableQualifier', location: 'mapping' })
  t.regex(error.message, /Oracle.*catalog/)
})
