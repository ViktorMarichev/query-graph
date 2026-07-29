import test from 'ava'

import {
  asc,
  coalesce,
  concat,
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  exists,
  isNull,
  lower,
  nullable,
  ordering,
  param,
  project,
  requiredParameter,
  relation,
  source,
  upper,
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

  t.is(definition.schemaVersion, 7)
  t.false('relations' in definition.projection.fields[0])
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

test('builds the supported semantic function expressions', (t) => {
  const root = source('root', {
    name: nullable('string'),
  })
  const expressions = [
    lower(root.field('name')),
    upper(root.field('name')),
    coalesce(root.field('name'), 'Unknown'),
    concat(root.field('name'), ' suffix'),
  ]

  t.deepEqual(
    expressions.map(({ name }) => name),
    ['lower', 'upper', 'coalesce', 'concat'],
  )
  t.deepEqual(expressions[2].arguments[1], {
    kind: 'literal',
    value: { kind: 'string', value: 'Unknown' },
  })
})

test('builds and compiles a typed exists constraint as a semijoin', (t) => {
  const staff = source('staff', {
    id: 'int64',
  })
  const staffService = source('staffService', {
    idStaff: 'int64',
    idService: 'int64',
  })
  const idService = requiredParameter('idService', 'int64')
  const hasService = exists(staffService, eq(staffService.field('idService'), param(idService)))
  const sourceIsTyped: Equal<typeof hasService.source, 'staffService'> = true
  t.true(sourceIsTyped)

  const definition = defineGraph({
    name: 'businessServiceSpecialists',
    root: staff,
    sources: [staff, staffService],
    parameters: [idService],
    relations: [
      relation('staffServices', staff, staffService, eq(staff.field('id'), staffService.field('idStaff')), {
        cardinality: 'many',
      }),
    ],
    constraints: [constraint('hasService', hasService)],
    projection: [project('id', staff.field('id'), { default: true })],
    orderings: [
      ordering({
        name: 'default',
        by: [asc(staff.field('id'))],
        default: true,
      }),
    ],
  })

  t.deepEqual(definition.constraints[0].predicate, {
    kind: 'exists',
    source: 'staffService',
    predicate: {
      kind: 'eq',
      left: { kind: 'field', source: 'staffService', field: 'idService' },
      right: { kind: 'parameter', name: 'idService' },
    },
  })

  const statement = registerDefinition(definition)
    .withRelationalMapping({
      sources: {
        staff: { table: 'Staff' },
        staffService: { table: 'BusinessServiceStaff' },
      },
    })
    .compileSqlServer({
      parameters: { idService: 42 },
      limit: 10,
    })

  t.true(statement.sql.includes('EXISTS ('))
  t.true(statement.sql.includes('FROM [BusinessServiceStaff] AS [t1]'))
  t.false(statement.sql.includes('FROM [Staff] AS [t0]\nINNER JOIN'))
  t.deepEqual(statement.relations, [])
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'idService', scalarType: 'int64' }])
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
  const childProjection = project('child.id', child.field('id'))
  const idOrdering = ordering({
    name: 'default',
    by: [asc(root.field('id'))],
    default: true,
  })

  const rootModule = defineGraphModule({
    name: 'root',
    sources: [root],
    projection: [idProjection],
    orderings: [idOrdering],
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
  t.deepEqual(definition.orderings, [idOrdering])
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
  t.deepEqual(statement.bindings, [{ name: 'p0', parameter: 'id', scalarType: 'int64' }])
  t.deepEqual(statement.columns, [{ name: 'c0', path: 'id', scalarType: 'int64', nullable: false, relations: [] }])
  t.deepEqual(statement.relations, [])
})
