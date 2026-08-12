import test from 'ava'

import {
  asc,
  constraint,
  defineGraph,
  desc,
  eq,
  nullable,
  ordering,
  param,
  project,
  projectObject,
  relation,
  requiredParameter,
  source,
} from '../definition.js'
import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

const link = source('link', {
  idOwner: 'int64',
  idValue: 'int64',
  order: 'int32',
})

const value = source('value', {
  id: 'int64',
  value: nullable('string'),
})

const idOwner = requiredParameter('idOwner', 'int64')
const valueRelation = relation('value', link, value, eq(link.field('idValue'), value.field('id')), { required: true })

const definition = defineGraph({
  name: 'attributeValues',
  root: link,
  sources: [link, value],
  parameters: [idOwner],
  relations: [valueRelation],
  constraints: [constraint(eq(link.field('idOwner'), param(idOwner)))],
  projection: [
    project('value.id', value.field('id'), {
      default: true,
    }),
    project('value.value', value.field('value'), {
      default: true,
    }),
  ],
  orderings: [
    ordering({
      name: 'default',
      by: [asc(link.field('order'))],
      default: true,
    }),
    ordering({
      name: 'valueDesc',
      by: [desc(value.field('id'))],
    }),
  ],
})

const mapping = {
  sources: {
    link: {
      table: { schema: 'dbo', name: 'ControllerAttributeValueLink' },
      columns: { idOwner: 'owner_id' },
    },
    value: {
      table: 'ControllerObjectValue',
    },
  },
}

test('compiles a mapped graph to SQL Server', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)

  const statement = graph.compileSqlServer({
    parameters: { idOwner: 42 },
    limit: 20,
  })

  t.is(graph.name, 'attributeValues')
  t.deepEqual(statement.columns, [
    { name: 'c0', path: 'value.id', scalarType: 'int64', nullable: false, relations: ['value'] },
    { name: 'c1', path: 'value.value', scalarType: 'string', nullable: true, relations: ['value'] },
  ])
  t.deepEqual(statement.relations, [
    {
      name: 'value',
      from: 'link',
      to: 'value',
      cardinality: 'one',
      required: true,
    },
  ])
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idOwner', scalarType: 'int64' }])
  t.regex(statement.sql, /\[t1\]\.\[id\] AS \[c0\]/)
  t.regex(statement.sql, /FROM \[dbo\]\.\[ControllerAttributeValueLink\] AS \[t0\]/)
  t.regex(statement.sql, /INNER JOIN \[ControllerObjectValue\] AS \[t1\]/)
  t.regex(statement.sql, /\(\[t0\]\.\[owner_id\] = @p0\)/)
  t.regex(statement.sql, /ORDER BY\n  \[t0\]\.\[order\] ASC/)
  t.regex(statement.sql, /OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY$/)
})

test('compiles the same mapped graph to Oracle', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)

  const statement = graph.compileOracle({
    parameters: { idOwner: 42 },
    limit: 20,
  })

  t.deepEqual(statement.columns[0], {
    name: 'c0',
    path: 'value.id',
    scalarType: 'int64',
    nullable: false,
    relations: ['value'],
  })
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idOwner', scalarType: 'int64' }])
  t.regex(statement.sql, /"t1"\."id" AS "c0"/)
  t.regex(statement.sql, /FROM "dbo"\."ControllerAttributeValueLink" "t0"/)
  t.regex(statement.sql, /INNER JOIN "ControllerObjectValue" "t1"/)
  t.regex(statement.sql, /\("t0"\."owner_id" = :p0\)/)
  t.regex(statement.sql, /ORDER BY\n  "t0"\."order" ASC/)
  t.regex(statement.sql, /OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY$/)
})

test('selects the same named ordering for SQL Server and Oracle', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const operation = {
    ordering: 'valueDesc' as const,
    parameters: { idOwner: 42 },
  }

  t.regex(graph.compileSqlServer(operation).sql, /ORDER BY\n  \[t1\]\.\[id\] DESC$/)
  t.regex(graph.compileOracle(operation).sql, /ORDER BY\n  "t1"\."id" DESC$/)
})

test('returns an unknown named ordering as an operation error', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const error = t.throws(() =>
    graph.compileSqlServer({
      // @ts-expect-error The runtime boundary must still reject untyped callers.
      ordering: 'missing',
      parameters: { idOwner: 42 },
    }),
  ) as QueryGraphError

  t.is(error.code, 'QUERY_GRAPH_OPERATION_INVALID')
  t.is(error.phase, 'operation')
  t.like(error.issues[0], {
    code: 'unknownOrdering',
    location: 'ordering',
  })
})

test('enforces compiler version capabilities', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)
  const operation = { parameters: { idOwner: 42 } }

  t.notRegex(graph.compileSqlServer(operation, { version: '2008' }).sql, /OFFSET/)
  t.notRegex(graph.compileOracle(operation, { version: '11g' }).sql, /OFFSET/)

  const sqlServerError = t.throws(() =>
    graph.compileSqlServer(
      { ...operation, limit: 20 },
      {
        version: '2008',
      },
    ),
  ) as QueryGraphError
  t.is(sqlServerError.code, 'QUERY_GRAPH_SQL_COMPILE_FAILED')
  t.is(sqlServerError.phase, 'sql')
  t.like(sqlServerError.issues[0], {
    code: 'unsupportedDialectFeature',
    location: 'sql',
  })

  const oracleError = t.throws(() =>
    graph.compileOracle(
      { ...operation, limit: 20 },
      {
        version: '11g',
      },
    ),
  ) as QueryGraphError
  t.is(oracleError.code, 'QUERY_GRAPH_SQL_COMPILE_FAILED')
  t.like(oracleError.issues[0], {
    code: 'unsupportedDialectFeature',
    location: 'sql',
  })

  const optionsError = t.throws(() =>
    graph.compileSqlServer(operation, {
      // @ts-expect-error This intentionally exercises the compiler options wire boundary.
      version: '2005',
    }),
  ) as QueryGraphError
  t.is(optionsError.code, 'QUERY_GRAPH_COMPILER_OPTIONS_WIRE_INVALID')
})

test('returns relational mapping errors to Node.js', (t) => {
  const graph = registerDefinition(definition)
  const error = t.throws(() =>
    graph.withRelationalMapping({
      sources: {
        missing: { table: 'Missing' },
      },
    }),
  ) as QueryGraphError

  t.is(error.code, 'QUERY_GRAPH_MAPPING_INVALID')
  t.is(error.phase, 'mapping')
  t.true(error.issues.some((issue) => issue.code === 'unknownSource'))
  t.true(error.issues.some((issue) => issue.code === 'missingSource'))
  t.regex(error.message, /UnknownSource/)
  t.regex(error.message, /MissingSource/)
})

test('returns projection object presence metadata to Node.js', (t) => {
  const users = source('presenceUsers', { id: 'int64' })
  const profiles = source('presenceProfiles', {
    id: 'int64',
    idUser: 'int64',
    name: nullable('string'),
  })
  const presenceDefinition = defineGraph({
    name: 'presenceMetadata',
    root: users,
    sources: [users, profiles],
    relations: [relation('profile', users, profiles, eq(users.field('id'), profiles.field('idUser')))],
    objects: [projectObject({ path: 'profile', presence: profiles.field('id') })],
    projection: [project('profile.name', profiles.field('name'))],
  })
  const graph = registerDefinition(presenceDefinition).withRelationalMapping({
    sources: {
      presenceUsers: { table: 'Users' },
      presenceProfiles: { table: 'Profiles' },
    },
  })

  const statement = graph.compileSqlServer({ select: ['profile.name'] })

  t.deepEqual(statement.objects, [{ path: 'profile', presenceColumn: 'o0' }])
  t.true(statement.sql.includes('[t1].[name] AS [c0],\n  [t1].[id] AS [o0]'))
})
