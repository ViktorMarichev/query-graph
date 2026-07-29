import test from 'ava'

import {
  constraint as functionalConstraint,
  defineGraph as functionalDefineGraph,
  dimension as functionalDimension,
  inParameter as functionalInParameter,
  measure as functionalMeasure,
  ordering as functionalOrdering,
  project as functionalProject,
  relation as functionalRelation,
} from '../definition.js'
import type { GraphDefinition } from '../definition.js'
import { registerDefinition } from '../index.js'
import {
  asc,
  constraint,
  count,
  defineGraph,
  dimension,
  eq,
  firstBy,
  inParameter,
  measure,
  ordering,
  project,
  relation,
  requiredListParameter,
  source,
} from '@query-graph/dsl-object'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

type ProjectionPathOf<Definition> =
  Definition extends GraphDefinition<infer _Parameter, infer ProjectionPath, infer _OrderingName>
    ? ProjectionPath
    : never

test('object DSL produces the canonical functional definition', (t) => {
  const staff = source('staff', { id: 'int64' })
  const personStaff = source('personStaff', {
    id: 'int64',
    idStaff: 'int64',
    idPerson: 'int64',
  })
  const ids = requiredListParameter('ids', 'int64')
  const on = eq(staff.field('id'), personStaff.field('idStaff'))
  const selection = firstBy(asc(personStaff.field('idPerson')), asc(personStaff.field('id')))

  const objectDefinition = defineGraph({
    name: 'staffCredentials',
    root: staff,
    sources: [staff, personStaff],
    parameters: [ids],
    relations: [
      relation({
        name: 'credentials',
        from: staff,
        to: personStaff,
        on,
        cardinality: 'one',
        selection,
      }),
    ],
    constraints: [
      constraint({
        name: 'ids',
        predicate: inParameter(staff.field('id'), ids),
        when: ids,
      }),
    ],
    projection: [
      project({ path: 'id', expression: staff.field('id'), default: true }),
      project({
        path: 'credentials.idPerson',
        expression: personStaff.field('idPerson'),
        default: true,
      }),
    ],
    orderings: [
      ordering({
        name: 'idAsc',
        by: [asc(staff.field('id'))],
        default: true,
      }),
    ],
  })

  const functionalDefinition = functionalDefineGraph({
    name: 'staffCredentials',
    root: staff,
    sources: [staff, personStaff],
    parameters: [ids],
    relations: [
      functionalRelation('credentials', staff, personStaff, on, {
        cardinality: 'one',
        selection,
      }),
    ],
    constraints: [
      functionalConstraint('ids', functionalInParameter(staff.field('id'), ids), {
        when: ids,
      }),
    ],
    projection: [
      functionalProject('id', staff.field('id'), { default: true }),
      functionalProject('credentials.idPerson', personStaff.field('idPerson'), {
        default: true,
      }),
    ],
    orderings: [
      functionalOrdering({
        name: 'idAsc',
        by: [asc(staff.field('id'))],
        default: true,
      }),
    ],
  })

  t.deepEqual(objectDefinition, functionalDefinition)
  t.is(registerDefinition(objectDefinition).name, 'staffCredentials')

  type ProjectionPath = ProjectionPathOf<typeof objectDefinition>
  const projectionPathsArePreserved: Equal<ProjectionPath, 'id' | 'credentials.idPerson'> = true
  t.true(projectionPathsArePreserved)
})

test('object DSL preserves summary field semantics', (t) => {
  const staff = source('staff', { id: 'int64' })

  t.deepEqual(
    dimension({ path: 'staff.id', expression: staff.field('id'), default: true }),
    functionalDimension('staff.id', staff.field('id'), { default: true }),
  )
  t.deepEqual(
    measure({ path: 'staff.count', expression: count(), default: true }),
    functionalMeasure('staff.count', count(), { default: true }),
  )
})

test('object DSL reports accidental positional structural calls', (t) => {
  const staff = source('staff', { id: 'int64' })
  const invalidObjectDslCalls = () => {
    // @ts-expect-error Object DSL relations accept one configuration object.
    relation('credentials', staff, staff, eq(staff.field('id'), staff.field('id')))
    // @ts-expect-error Object DSL projections accept one configuration object.
    project('id', staff.field('id'))
  }
  const positionalRelation = relation as unknown as (...arguments_: unknown[]) => unknown
  const error = t.throws(() => positionalRelation('credentials'))

  t.is(typeof invalidObjectDslCalls, 'function')
  t.is(error.message, 'relation expects a configuration object')
})

test('object DSL rejects unknown structural configuration fields', (t) => {
  const staff = source('staff', { id: 'int64' })
  const predicate = eq(staff.field('id'), staff.field('id'))

  const invalidRelation = {
    name: 'self',
    from: staff,
    to: staff,
    on: predicate,
    cardinallity: 'many',
  }
  const invalidConstraint = {
    name: 'active',
    predicate,
    wheen: 'id',
  }
  const invalidProject = {
    path: 'id',
    expression: staff.field('id'),
    deafult: true,
  }
  const invalidDimension = {
    path: 'id',
    expression: staff.field('id'),
    selectedByDefault: true,
  }
  const invalidMeasure = {
    path: 'count',
    expression: count(),
    distinct: true,
  }
  const invalidOrdering = {
    name: 'idAsc',
    by: [asc(staff.field('id'))] as const,
    deafult: true,
  }

  const cases = [
    {
      factory: 'relation',
      field: 'cardinallity',
      invoke: () => relation(invalidRelation),
    },
    {
      factory: 'constraint',
      field: 'wheen',
      invoke: () => constraint(invalidConstraint),
    },
    {
      factory: 'project',
      field: 'deafult',
      invoke: () => project(invalidProject),
    },
    {
      factory: 'dimension',
      field: 'selectedByDefault',
      invoke: () => dimension(invalidDimension),
    },
    {
      factory: 'measure',
      field: 'distinct',
      invoke: () => measure(invalidMeasure),
    },
    {
      factory: 'ordering',
      field: 'deafult',
      invoke: () => ordering(invalidOrdering),
    },
  ]

  for (const { factory, field, invoke } of cases) {
    const error = t.throws(invoke)
    t.is(error.message, `${factory} received unknown configuration field ${JSON.stringify(field)}`)
  }
})

test('object DSL validates ordering configuration before it reaches Rust', (t) => {
  const staff = source('staff', { id: 'int64' })
  const byId = asc(staff.field('id'))
  const invoke = ordering as unknown as (configuration: Record<string, unknown>) => unknown

  t.is(t.throws(() => invoke({ name: '', by: [byId] })).message, 'ordering name must be a non-empty string')
  t.is(
    t.throws(() => invoke({ name: 'idAsc', by: [] })).message,
    'ordering by must contain at least one order expression',
  )
  t.is(
    t.throws(() => invoke({ name: 'idAsc', by: [byId], default: 'yes' })).message,
    'ordering default must be a boolean',
  )
})
