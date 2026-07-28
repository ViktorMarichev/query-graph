import test from 'ava'

import {
  asc,
  constraint,
  defineGraph,
  eq,
  firstBy,
  inParameter,
  optionalListParameter,
  param,
  project,
  relation,
  requiredListParameter,
  requiredParameter,
  source,
} from '../definition.js'
import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

test('builds typed list parameter expressions without scalar/list ambiguity', (t) => {
  const staff = source('staff', { id: 'int64' })
  const ids = requiredListParameter('ids', 'int64')
  const optionalIds = optionalListParameter('optionalIds', 'int64')
  const id = requiredParameter('id', 'int64')
  const membership = inParameter(staff.field('id'), ids)

  t.deepEqual(ids, {
    name: 'ids',
    scalarType: 'int64',
    shape: 'list',
    required: true,
  })
  t.deepEqual(optionalIds, {
    name: 'optionalIds',
    scalarType: 'int64',
    shape: 'list',
  })
  t.deepEqual(membership, {
    kind: 'inParameter',
    expression: { kind: 'field', source: 'staff', field: 'id' },
    parameter: 'ids',
  })

  const invalidParameterExpressions = () => {
    // @ts-expect-error A list parameter is not a scalar expression.
    param(ids)
    // @ts-expect-error A scalar parameter cannot back an inParameter expression.
    inParameter(staff.field('id'), id)
  }
  t.is(typeof invalidParameterExpressions, 'function')
})

test('expands list parameters and exposes element indices to the consumer', (t) => {
  const staff = source('staff', { id: 'int64' })
  const ids = requiredListParameter('ids', 'int64')
  const definition = defineGraph({
    name: 'staffByIds',
    root: staff,
    sources: [staff],
    parameters: [ids],
    constraints: [constraint('ids', inParameter(staff.field('id'), ids))],
    projection: [project('id', staff.field('id'), { default: true })],
  })
  const graph = registerDefinition(definition).withRelationalMapping({
    sources: { staff: { table: 'Staff' } },
  })

  const statement = graph.compileSqlServer({
    parameters: { ids: [12, 18, 24] },
  })

  t.regex(statement.sql, /\[t0\]\.\[id\] IN \(@p0, @p1, @p2\)/)
  t.deepEqual(statement.bindings, [
    { name: 'p0', parameter: 'ids', scalarType: 'int64', index: 0 },
    { name: 'p1', parameter: 'ids', scalarType: 'int64', index: 1 },
    { name: 'p2', parameter: 'ids', scalarType: 'int64', index: 2 },
  ])
  const empty = graph.compileOracle({ parameters: { ids: [] } })
  t.regex(empty.sql, /\(1 = 0\)/)
  t.deepEqual(empty.bindings, [])
})

test('builds a database-independent firstBy relation selection', (t) => {
  const staff = source('staff', { id: 'int64' })
  const personStaff = source('personStaff', {
    id: 'int64',
    idStaff: 'int64',
    idPerson: 'int64',
  })
  const credentials = relation('credentials', staff, personStaff, eq(staff.field('id'), personStaff.field('idStaff')), {
    selection: firstBy(asc(personStaff.field('idPerson')), asc(personStaff.field('id'))),
  })
  const definition = defineGraph({
    name: 'staffCredentials',
    root: staff,
    sources: [staff, personStaff],
    relations: [credentials],
    projection: [
      project('id', staff.field('id'), { default: true }),
      project('credentials.idPerson', personStaff.field('idPerson'), { default: true }),
    ],
  })
  const graph = registerDefinition(definition).withRelationalMapping({
    sources: {
      staff: { table: 'Staff' },
      personStaff: { table: 'PersonStaff' },
    },
  })

  t.deepEqual(definition.relations[0].selection, {
    kind: 'firstBy',
    orderBy: [
      {
        expression: { kind: 'field', source: 'personStaff', field: 'idPerson' },
        direction: 'asc',
      },
      {
        expression: { kind: 'field', source: 'personStaff', field: 'id' },
        direction: 'asc',
      },
    ],
  })

  const sqlServer = graph.compileSqlServer({})
  t.regex(sqlServer.sql, /OUTER APPLY \(/)
  t.regex(sqlServer.sql, /SELECT TOP \(1\) \[t1\]\.\*/)
  t.regex(sqlServer.sql, /ORDER BY \[t1\]\.\[idPerson\] ASC, \[t1\]\.\[id\] ASC/)

  const oracle = graph.compileOracle({})
  t.regex(oracle.sql, /OUTER APPLY \(/)
  t.regex(oracle.sql, /FETCH FIRST 1 ROW ONLY/)

  const oracle11gError = t.throws(() => graph.compileOracle({}, { version: '11g' })) as QueryGraphError
  t.is(oracle11gError.code, 'QUERY_GRAPH_SQL_COMPILE_FAILED')
  t.regex(oracle11gError.message, /firstBy relation selection/)
})
