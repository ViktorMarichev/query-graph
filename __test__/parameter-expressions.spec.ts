import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  constraint,
  defineGraph,
  eq,
  inParam,
  inParameter,
  like,
  optionalParam,
  optionalParameter,
  param,
  project,
  requiredListParam,
  requiredListParameter,
  requiredParam,
  requiredParameter,
  source,
} from '../dsl.js'
import type { ExpressionNullability, ExpressionScalarType } from '../dsl.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

test('promotes scalar parameter refs to expressions', (t) => {
  const users = source('users', {
    id: 'int64',
    organisationId: 'int64',
    name: 'string',
  })
  const organisationId = requiredParam('organisationId', 'int64')
  const search = optionalParam('search', 'string')

  t.is(requiredParam, requiredParameter)
  t.is(optionalParam, optionalParameter)

  const organisationPredicate = eq(users.field('organisationId'), organisationId)
  const searchPredicate = like(users.field('name'), search)

  t.deepEqual(organisationPredicate, eq(users.field('organisationId'), param(organisationId)))
  t.deepEqual(searchPredicate, {
    kind: 'like',
    expression: { kind: 'field', source: 'users', field: 'name' },
    pattern: { kind: 'parameter', name: 'search' },
  })

  const definition = defineGraph({
    name: 'users',
    root: users,
    sources: [users],
    parameters: [organisationId, search],
    constraints: [
      constraint({ predicate: organisationPredicate }),
      constraint({ predicate: searchPredicate, when: search }),
    ],
    projection: [project({ path: 'id', expression: users.field('id'), default: true })],
  })
  const graph = registerDefinition(definition).withRelationalMapping({
    sources: { users: { table: 'Users' } },
  })

  const statement = graph.compileSqlServer({
    parameters: { organisationId: 7, search: 'Ann%' },
  })
  t.deepEqual(
    statement.bindings.map(({ parameter }) => parameter),
    ['organisationId', 'search'],
  )

  const statementWithoutSearch = graph.compileSqlServer({
    parameters: { organisationId: 7 },
  })
  t.deepEqual(
    statementWithoutSearch.bindings.map(({ parameter }) => parameter),
    ['organisationId'],
  )

  const scalarTypeIsPreserved: Equal<ExpressionScalarType<typeof organisationId>, 'int64'> = true
  const parameterIsNotNullable: Equal<ExpressionNullability<typeof organisationId>, false> = true
  t.true(scalarTypeIsPreserved)
  t.true(parameterIsNotNullable)
})

test('rejects list and unbranded parameter-like objects in scalar expressions', (t) => {
  const users = source('users', { id: 'int64' })
  const ids = requiredListParam('ids', 'int64')

  t.is(requiredListParam, requiredListParameter)
  t.is(inParam, inParameter)
  t.deepEqual(inParam(users.field('id'), ids), inParameter(users.field('id'), ids))

  const invalidTypeUsage = () => {
    // @ts-expect-error A list parameter is not a scalar expression.
    eq(users.field('id'), ids)
  }
  t.is(typeof invalidTypeUsage, 'function')

  const listError = t.throws(() => eq(users.field('id'), ids as never), { instanceOf: TypeError })
  t.regex(listError.message, /List parameter ids cannot be used as a scalar expression/)

  const lookalikeError = t.throws(() => eq(users.field('id'), { name: 'id', scalarType: 'int64' } as never), {
    instanceOf: TypeError,
  })
  t.regex(lookalikeError.message, /Literal must be null, boolean, string, or number/)
})
