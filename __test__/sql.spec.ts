import test from 'ava'

import {
  asc,
  constraint,
  defineGraph,
  eq,
  nullable,
  param,
  project,
  relation,
  requiredParameter,
  source,
} from '../definition.js'
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
  constraints: [constraint('owner', eq(link.field('idOwner'), param(idOwner)))],
  projection: [
    project('value.id', value.field('id'), {
      through: [valueRelation],
      default: true,
    }),
    project('value.value', value.field('value'), {
      through: [valueRelation],
      default: true,
    }),
  ],
  defaultOrderBy: [asc(link.field('order'))],
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
    { name: 'c0', path: 'value.id', relations: ['value'] },
    { name: 'c1', path: 'value.value', relations: ['value'] },
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
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idOwner', scalarType: 'int64', cardinality: 'one' }])
  t.regex(statement.sql, /\[t1\]\.\[id\] AS \[c0\]/)
  t.regex(statement.sql, /FROM \[dbo\]\.\[ControllerAttributeValueLink\] AS \[t0\]/)
  t.regex(statement.sql, /INNER JOIN \[ControllerObjectValue\] AS \[t1\]/)
  t.regex(statement.sql, /\(\[t0\]\.\[owner_id\] = @p0\)/)
  t.regex(statement.sql, /OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY$/)
})

test('compiles the same mapped graph to Oracle', (t) => {
  const graph = registerDefinition(definition).withRelationalMapping(mapping)

  const statement = graph.compileOracle({
    parameters: { idOwner: 42 },
    limit: 20,
  })

  t.deepEqual(statement.columns[0], { name: 'c0', path: 'value.id', relations: ['value'] })
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idOwner', scalarType: 'int64', cardinality: 'one' }])
  t.regex(statement.sql, /"t1"\."id" AS "c0"/)
  t.regex(statement.sql, /FROM "dbo"\."ControllerAttributeValueLink" "t0"/)
  t.regex(statement.sql, /INNER JOIN "ControllerObjectValue" "t1"/)
  t.regex(statement.sql, /\("t0"\."owner_id" = :p0\)/)
  t.regex(statement.sql, /OFFSET 0 ROWS FETCH NEXT 20 ROWS ONLY$/)
})

test('returns relational mapping errors to Node.js', (t) => {
  const graph = registerDefinition(definition)
  const error = t.throws(() =>
    graph.withRelationalMapping({
      sources: {
        missing: { table: 'Missing' },
      },
    }),
  )

  t.regex(error.message, /UnknownSource/)
  t.regex(error.message, /MissingSource/)
})
