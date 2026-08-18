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
  and,
  asc,
  constraint,
  count,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  dimension,
  eq,
  exists,
  fieldType,
  firstBy,
  inParameter,
  measure,
  ordering,
  project,
  projectObject,
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
    constraints: [functionalConstraint(functionalInParameter(staff.field('id'), ids), { when: ids })],
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
  t.false('name' in objectDefinition.constraints[0])
  t.is(registerDefinition(objectDefinition).name, 'staffCredentials')

  type ProjectionPath = ProjectionPathOf<typeof objectDefinition>
  const projectionPathsArePreserved: Equal<ProjectionPath, 'id' | 'credentials.idPerson'> = true
  t.true(projectionPathsArePreserved)
})

test('object DSL compiles an anchored exists inside a firstBy relation', (t) => {
  const users = source('users', { id: 'int64' })
  const profiles = source('profiles', {
    id: 'int64',
    idUser: 'int64',
  })
  const profileFlags = source('profileFlags', {
    idProfile: 'int64',
    enabled: 'boolean',
  })

  const definition = defineGraph({
    name: 'usersWithPreferredProfile',
    root: users,
    sources: [users, profiles, profileFlags],
    relations: [
      relation({
        name: 'profile',
        from: users,
        to: profiles,
        on: and(
          eq(users.field('id'), profiles.field('idUser')),
          exists(profileFlags, eq(profileFlags.field('enabled'), true), {
            from: profiles,
          }),
        ),
        selection: firstBy(asc(profiles.field('id'))),
      }),
      relation({
        name: 'profileFlags',
        from: profiles,
        to: profileFlags,
        on: eq(profiles.field('id'), profileFlags.field('idProfile')),
        cardinality: 'many',
      }),
    ],
    projection: [
      project({ path: 'id', expression: users.field('id'), default: true }),
      project({
        path: 'profile.id',
        expression: profiles.field('id'),
        default: true,
      }),
    ],
  })

  const statement = registerDefinition(definition)
    .withRelationalMapping({
      sources: {
        users: { table: 'Users' },
        profiles: { table: 'Profiles' },
        profileFlags: { table: 'ProfileFlags' },
      },
    })
    .compileSqlServer({})

  t.true(statement.sql.includes('OUTER APPLY ('))
  t.true(statement.sql.includes('AND EXISTS ('))
  t.true(statement.sql.includes('FROM [ProfileFlags] AS [t2]'))
  t.deepEqual(
    statement.relations.map(({ name }) => name),
    ['profile'],
  )
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
    predicate,
    wheen: 'id',
  }
  const namedConstraint = {
    name: 'active',
    predicate,
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
      factory: 'constraint',
      field: 'name',
      invoke: () => constraint(namedConstraint),
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

test('object DSL rejects unknown graph configuration fields', (t) => {
  const staff = source('staff', { id: 'int64' })
  const id = project({ path: 'id', expression: staff.field('id') })
  const amount = dimension({ path: 'amount', expression: staff.field('id') })

  const graphError = t.throws(() =>
    defineGraph({
      name: 'staff',
      root: staff,
      sources: [staff],
      projection: [id],
      projecton: [id],
    } as never),
  )
  const moduleError = t.throws(() =>
    defineGraphModule({
      name: 'staffModule',
      sources: [staff],
      constaints: [],
    } as never),
  )
  const summaryError = t.throws(() =>
    defineSummaryGraph({
      name: 'staffSummary',
      root: staff,
      sources: [staff],
      dimensions: [amount],
      meassures: [],
    } as never),
  )

  t.regex(graphError.message, /defineGraph received unknown configuration field "projecton"/)
  t.regex(moduleError.message, /defineGraphModule received unknown configuration field "constaints"/)
  t.regex(summaryError.message, /defineSummaryGraph received unknown configuration field "meassures"/)
})

test('object DSL rejects option values that would otherwise change defaults', (t) => {
  const staff = source('staff', { id: 'int64' })
  const predicate = eq(staff.field('id'), staff.field('id'))

  const invalidCardinality = t.throws(() =>
    relation({
      name: 'self',
      from: staff,
      to: staff,
      on: predicate,
      cardinality: 'single',
    } as never),
  )
  const invalidRequired = t.throws(() =>
    relation({
      name: 'self',
      from: staff,
      to: staff,
      on: predicate,
      required: 'yes',
    } as never),
  )
  const invalidDefault = t.throws(() => project({ path: 'id', expression: staff.field('id'), default: 'yes' } as never))
  const invalidFieldOption = t.throws(() => fieldType('string', { nullable: 'yes' } as never))
  const invalidFieldSpecification = t.throws(() =>
    source('invalid', { value: { scalarType: 'string', nullabel: true } } as never),
  )
  const invalidNulls = t.throws(() => asc(staff.field('id'), { nulls: 'middle' } as never))

  t.regex(invalidCardinality.message, /relation cardinality/)
  t.is(invalidRequired.message, 'relation required must be a boolean')
  t.is(invalidDefault.message, 'project default must be a boolean')
  t.is(invalidFieldOption.message, 'fieldType nullable must be a boolean')
  t.regex(invalidFieldSpecification.message, /unknown configuration field "nullabel"/)
  t.regex(invalidNulls.message, /asc nulls/)
})

test('object DSL defines and composes projection object presence', (t) => {
  const users = source('projectionObjectUsers', { id: 'int64' })
  const profiles = source('projectionObjectProfiles', {
    id: 'int64',
    idUser: 'int64',
    name: 'string',
  })
  const profileObject = projectObject({
    path: 'profile',
    presence: profiles.field('id'),
  })
  const profileModule = defineGraphModule({
    name: 'projectionObjectProfile',
    sources: [profiles],
    objects: [profileObject],
    projection: [project({ path: 'profile.name', expression: profiles.field('name') })],
  })

  const definition = defineGraph({
    name: 'projectionObjects',
    root: users,
    modules: [profileModule],
    sources: [users],
    relations: [
      relation({
        name: 'profile',
        from: users,
        to: profiles,
        on: eq(users.field('id'), profiles.field('idUser')),
      }),
    ],
  })

  t.deepEqual(definition.projection.objects, [
    {
      path: ['profile'],
      presence: {
        kind: 'field',
        source: 'projectionObjectProfiles',
        field: 'id',
      },
    },
  ])

  const invalid = {
    path: 'profile',
    presence: profiles.field('id'),
    presense: profiles.field('id'),
  }
  const error = t.throws(() => projectObject(invalid))
  t.is(error.message, 'projectObject received unknown configuration field "presense"')
})
