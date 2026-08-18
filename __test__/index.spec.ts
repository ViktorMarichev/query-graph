import test from 'ava'

import {
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
import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'

const link = source('link', {
  id: 'int64',
  idOwner: 'int64',
  idAttributeValue: 'int64',
})

const value = source('value', {
  id: 'int64',
  value: nullable('string'),
})

const idOwner = requiredParameter('idOwner', 'int64')
const valueRelation = relation('value', link, value, eq(link.field('idAttributeValue'), value.field('id')), {
  required: true,
})

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
    project('value.value', value.field('value')),
  ],
})

test('registers a definition as a native graph handle', (t) => {
  const graph = registerDefinition(definition)

  t.is(graph.name, 'attributeValues')
  t.is(graph.root, 'link')
  t.is(graph.sourceCount, 2)
  t.is(graph.relationCount, 1)
  t.true(graph.hasSource('value'))
  t.true(graph.hasField('value', 'value'))
  t.true(graph.hasParameter('idOwner'))
  t.true(graph.hasRelation('value'))
  t.deepEqual(graph.selectableFields(), ['value.id', 'value.value'])
})

test('returns structured definition validation errors to Node.js', (t) => {
  const error = t.throws(() =>
    registerDefinition({
      ...definition,
      root: 'missing',
    }),
  ) as QueryGraphError

  t.is(error.name, 'QueryGraphError')
  t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
  t.is(error.phase, 'definition')
  t.deepEqual(error.issues, [
    {
      code: 'unknownRoot',
      location: 'root',
      message: 'root source "missing" is not defined',
    },
  ])
  t.regex(error.message, /UnknownRoot/)
})

test('returns structured expression type errors to Node.js', (t) => {
  const invalidDefinition = defineGraph({
    name: 'invalidTypes',
    root: link,
    sources: [link],
    constraints: [constraint(eq(link.field('idOwner'), 'not an id'))],
    projection: [project('id', link.field('id'), { default: true })],
  })

  const error = t.throws(() => registerDefinition(invalidDefinition)) as QueryGraphError

  t.is(error.code, 'QUERY_GRAPH_DEFINITION_INVALID')
  t.like(error.issues[0], {
    code: 'incompatibleExpressionTypes',
    location: 'constraints[0].predicate',
  })
  t.regex(error.message, /equality comparison cannot combine int64 and string/)
})
