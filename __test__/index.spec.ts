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
import { registerDefinition } from '../index.js'

const link = source('link', {
  id: 'int64',
  idOwner: 'int64',
  idControllerObjectValue: 'int64',
})

const value = source('value', {
  id: 'int64',
  value: nullable('string'),
})

const idOwner = requiredParameter('idOwner', 'int64')
const valueRelation = relation('value', link, value, eq(link.field('idControllerObjectValue'), value.field('id')), {
  required: true,
})

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
    }),
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

test('returns definition validation errors to Node.js', (t) => {
  const error = t.throws(() =>
    registerDefinition({
      ...definition,
      root: 'missing',
    }),
  )

  t.regex(error.message, /UnknownRoot/)
  t.regex(error.message, /root source "missing" is not defined/)
})
