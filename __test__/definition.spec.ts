import test from 'ava'

import {
  asc,
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  isNull,
  nullable,
  param,
  project,
  requiredParameter,
  relation,
  source,
} from '../definition.js'
import { registerDefinition } from '../index.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

test('builds the versioned wire definition without author-written discriminators', (t) => {
  const root = source('root', {
    id: 'int64',
    dateDelete: nullable('dateTime'),
  })
  const id = requiredParameter('id', 'int64')

  const fieldsAreTyped: Equal<Parameters<typeof root.field>[0], 'id' | 'dateDelete'> = true
  t.true(fieldsAreTyped)

  const definition = defineGraph({
    name: 'dslDefinition',
    root,
    sources: [root],
    parameters: [id],
    constraints: [
      constraint('id', eq(root.field('id'), param(id))),
      constraint('active', isNull(root.field('dateDelete'))),
    ],
    projection: [project('id', root.field('id'), { default: true })],
  })

  t.is(definition.schemaVersion, 1)
  t.deepEqual(definition.constraints[0].predicate, {
    kind: 'eq',
    left: { kind: 'field', source: 'root', field: 'id' },
    right: { kind: 'parameter', name: 'id' },
  })
  t.false(Object.keys(root).includes('field'))
  t.false('field' in definition.sources[0])
  const error = t.throws(() => root.field('missing' as never))
  t.is(error.message, 'Source root does not define field missing')
})

test('composes graph modules into a flat wire definition', (t) => {
  const root = source('root', {
    id: 'int64',
    dateDelete: nullable('dateTime'),
  })
  const child = source('child', {
    id: 'int64',
    idRoot: 'int64',
  })
  const id = requiredParameter('id', 'int64')
  const idProjection = project('id', root.field('id'), { default: true })
  const childRelation = relation('child', root, child, eq(root.field('id'), child.field('idRoot')))
  const childProjection = project('child.id', child.field('id'), { through: [childRelation] })
  const idOrder = asc(root.field('id'))

  const rootModule = defineGraphModule({
    name: 'root',
    sources: [root],
    projection: [idProjection],
    defaultOrderBy: [idOrder],
  })
  const filterModule = defineGraphModule({
    name: 'filter',
    sources: [root, child],
    parameters: [id],
    relations: [childRelation],
    constraints: [
      constraint('id', eq(root.field('id'), param(id))),
      constraint('active', isNull(root.field('dateDelete'))),
    ],
    projection: [childProjection],
  })
  const combinedModule = defineGraphModule({
    name: 'combined',
    modules: [rootModule, filterModule],
  })

  const definition = defineGraph({
    name: 'composed',
    root,
    modules: [combinedModule, rootModule],
  })

  t.deepEqual(
    definition.sources.map((definitionSource) => definitionSource.key),
    ['root', 'child'],
  )
  t.deepEqual(
    definition.parameters.map((parameter) => parameter.name),
    ['id'],
  )
  t.deepEqual(
    definition.constraints.map((definitionConstraint) => definitionConstraint.name),
    ['id', 'active'],
  )
  t.deepEqual(
    definition.relations.map((definitionRelation) => definitionRelation.name),
    ['child'],
  )
  t.deepEqual(
    definition.projection.fields.map((field) => field.path),
    [['id'], ['child', 'id']],
  )
  t.deepEqual(definition.defaultOrderBy, [idOrder])
  t.false('modules' in definition)
  t.true(Object.isFrozen(combinedModule))
  t.true(Object.isFrozen(combinedModule.sources))
  t.true(Object.isFrozen(combinedModule.sources[0]))
  t.true(Object.isFrozen(combinedModule.sources[0].fields))
  t.true(Object.isFrozen(combinedModule.relations[0].on))
  t.true(Object.isFrozen(definition))
  t.true(Object.isFrozen(definition.projection.fields))
})

test('rejects conflicting definitions from graph modules', (t) => {
  const root = source('root', { id: 'int64' })
  const conflictingRoot = source('root', { code: 'string' })
  const rootModule = defineGraphModule({
    name: 'root',
    sources: [root],
  })
  const conflictingModule = defineGraphModule({
    name: 'conflicting',
    sources: [conflictingRoot],
  })

  const error = t.throws(() =>
    defineGraph({
      name: 'conflict',
      root,
      modules: [rootModule, conflictingModule],
    }),
  )

  t.is(error.message, 'Conflicting source "root" from graph module "root" and graph module "conflicting"')
})

test('passes a DSL definition to Rust without an intermediate compiler', (t) => {
  const root = source('root', { id: 'int64' })
  const id = requiredParameter('id', 'int64')
  const rootModule = defineGraphModule({
    name: 'root',
    sources: [root],
    parameters: [id],
    constraints: [constraint('id', eq(root.field('id'), param(id)))],
    projection: [project('id', root.field('id'), { default: true })],
  })
  const definition = defineGraph({
    name: 'dslIntegration',
    root,
    modules: [rootModule],
  })

  const statement = registerDefinition(definition)
    .withRelationalMapping({
      sources: {
        root: { table: 'Root' },
      },
    })
    .compileSqlServer({
      parameters: { id: 42 },
    })

  t.regex(statement.sql, /\[t0\]\.\[id\] AS \[c0\]/)
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'id', scalarType: 'int64', cardinality: 'one' }])
  t.deepEqual(statement.columns, [{ name: 'c0', path: 'id', relations: [] }])
  t.deepEqual(statement.relations, [])
})
