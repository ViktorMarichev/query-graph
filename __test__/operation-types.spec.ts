import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  asc,
  batchQuery,
  batchRelation,
  composeGraph,
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  exists,
  firstBy,
  inParameter,
  like,
  optionalListParameter,
  optionalParameter,
  ordering,
  param,
  project,
  relation,
  requiredListParameter,
  requiredParameter,
  source,
} from '../definition.js'
import type { GraphDefinitionInput, RelationalQueryGraph } from '../definition.js'

test('propagates definition parameters and projection paths to query operations', (t) => {
  const staff = source('staff', {
    id: 'int64',
    idOrganisation: 'int64',
    name: 'string',
  })
  const personStaff = source('personStaff', {
    id: 'int64',
    idStaff: 'int64',
    idPerson: 'int64',
  })

  const idOrganisation = requiredParameter('idOrganisation', 'int64')
  const search = optionalParameter('search', 'string')
  const personIds = optionalListParameter('personIds', 'int64')

  const staffModule = defineGraphModule({
    name: 'staff',
    sources: [staff],
    parameters: [idOrganisation],
    constraints: [constraint(eq(staff.field('idOrganisation'), param(idOrganisation)))],
    projection: [project('id', staff.field('id'), { default: true })],
    orderings: [
      ordering({
        name: 'idAsc',
        by: [asc(staff.field('id'))],
        default: true,
      }),
    ],
  })

  const credentials = relation('credentials', staff, personStaff, eq(staff.field('id'), personStaff.field('idStaff')), {
    cardinality: 'one',
    selection: firstBy(asc(personStaff.field('id'))),
  })

  const definition = defineGraph({
    name: 'staffCredentials',
    root: staff,
    modules: [staffModule],
    sources: [personStaff],
    parameters: [search, personIds],
    relations: [credentials],
    constraints: [
      constraint(like(staff.field('name'), param(search)), { when: search }),
      constraint(exists(personStaff, inParameter(personStaff.field('idPerson'), personIds)), {
        when: personIds,
      }),
    ],
    projection: [
      project(['credentials', 'idPerson'], personStaff.field('idPerson'), {
        default: true,
      }),
    ],
    orderings: [
      ordering({
        name: 'nameAsc',
        by: [asc(staff.field('name'))],
      }),
    ],
  })

  const graph = registerDefinition(definition)
  const relationalGraph = graph.withRelationalMappings([
    {
      sources: {
        staff: { table: 'Staff' },
      },
    },
    {
      sources: {
        personStaff: { table: 'PersonStaff' },
      },
    },
  ])

  relationalGraph.compileSqlServer({
    select: ['id', 'credentials.idPerson'],
    ordering: 'nameAsc',
    parameters: {
      idOrganisation: 12,
      search: 'Ann%',
      personIds: [4, '8'],
    },
  })
  relationalGraph.compileOracle({
    parameters: {
      idOrganisation: '12',
    },
  })

  const selectableField: 'id' | 'credentials.idPerson' = graph.selectableFields()[0]
  t.truthy(selectableField)

  const invalidOperations = () => {
    // @ts-expect-error A required parameter makes the parameters property mandatory.
    relationalGraph.compileSqlServer({})

    relationalGraph.compileSqlServer({
      // @ts-expect-error idOrganisation is required.
      parameters: { search: 'Ann%' },
    })

    relationalGraph.compileSqlServer({
      parameters: {
        idOrganisation: 12,
        // @ts-expect-error A string parameter does not accept a number.
        search: 42,
      },
    })

    relationalGraph.compileSqlServer({
      parameters: {
        idOrganisation: 12,
        // @ts-expect-error A list parameter requires an array.
        personIds: 4,
      },
    })

    relationalGraph.compileSqlServer({
      parameters: {
        idOrganisation: 12,
        // @ts-expect-error Every list element must match its scalar type.
        personIds: [4, false],
      },
    })

    relationalGraph.compileSqlServer({
      // @ts-expect-error Selection is restricted to declared projection paths.
      select: ['credentials.unknown'],
      parameters: { idOrganisation: 12 },
    })

    relationalGraph.compileSqlServer({
      // @ts-expect-error Ordering is restricted to names declared by the graph and its modules.
      ordering: 'missing',
      parameters: { idOrganisation: 12 },
    })

    relationalGraph.compileSqlServer({
      parameters: {
        idOrganisation: 12,
        // @ts-expect-error Unknown parameters are rejected.
        unknown: true,
      },
    })
  }

  t.is(typeof invalidOperations, 'function')
})

test('keeps parameters optional for a parameterless definition', (t) => {
  const staff = source('staff', { id: 'int64' })
  const definition = defineGraph({
    name: 'allStaff',
    root: staff,
    sources: [staff],
    projection: [project('id', staff.field('id'), { default: true })],
  })
  const graph = registerDefinition(definition).withRelationalMapping({
    sources: { staff: { table: 'Staff' } },
  })

  const statement = graph.compileSqlServer({ select: ['id'] })
  t.truthy(statement.sql)

  const invalidOperation = () => {
    graph.compileSqlServer({
      // @ts-expect-error A parameterless definition does not accept parameters.
      parameters: { unknown: true },
    })
  }
  t.is(typeof invalidOperation, 'function')
})

test('preserves parameter contracts across ordinary and composed operations', (t) => {
  const item = source('item', { id: 'int64' })
  const base = {
    name: 'parameterContracts',
    root: item,
    sources: [item],
    projection: [project('id', item.field('id'), { default: true })],
  }
  const parameterlessDefinition = defineGraph(base)
  const optionalDefinition = defineGraph({
    ...base,
    parameters: [optionalParameter('kind', 'string'), optionalListParameter('ids', 'int64')],
  })
  const ids = requiredListParameter('ids', 'int64')
  const childDefinition = defineGraph({
    ...base,
    parameters: [ids, requiredParameter('kind', 'string'), optionalListParameter('labels', 'string')],
  })

  // These calls are checked by tsc without asking the runtime to accept invalid operations.
  const check = (
    parameterless: RelationalQueryGraph<typeof parameterlessDefinition>,
    optional: RelationalQueryGraph<typeof optionalDefinition>,
    child: RelationalQueryGraph<typeof childDefinition>,
    broad: RelationalQueryGraph<GraphDefinitionInput>,
  ) => {
    const noParameters = composeGraph({ root: parameterless, relations: [] })
    noParameters.compileSqlServerPlan({})
    // @ts-expect-error A parameterless composed definition rejects parameters.
    noParameters.compileSqlServerPlan({ parameters: { unknown: true } })

    const optionalParameters = composeGraph({ root: optional, relations: [] })
    const optionalValues = { kind: 'preview', ids: [1, '2'] as const }
    optional.compileSqlServer({})
    optional.compileSqlServer({ parameters: {} })
    optional.compileSqlServer({ parameters: optionalValues })
    optionalParameters.compileSqlServerPlan({})
    optionalParameters.compileSqlServerPlan({ parameters: {} })
    optionalParameters.compileSqlServerPlan({ parameters: optionalValues })
    // @ts-expect-error Optional composed scalar parameters retain their scalar type.
    optionalParameters.compileSqlServerPlan({ parameters: { kind: 1 } })
    // @ts-expect-error Optional composed list parameters require an array.
    optionalParameters.compileSqlServerPlan({ parameters: { ids: 1 } })
    // @ts-expect-error Optional composed parameters reject unknown names.
    optionalParameters.compileSqlServerPlan({ parameters: { unknown: true } })

    const requiredParameters = composeGraph({ root: child, relations: [] })
    const childValues = { ids: [1, '2'] as const, kind: 'preview', labels: ['a', 'b'] as const }
    child.compileSqlServer({ parameters: childValues })
    requiredParameters.compileSqlServerPlan({ parameters: childValues })
    // @ts-expect-error Required composed parameters make the property mandatory.
    requiredParameters.compileSqlServerPlan({})
    // @ts-expect-error Supplying the list does not satisfy another required parameter.
    requiredParameters.compileSqlServerPlan({ parameters: { ids: [1] } })
    // @ts-expect-error Every composed list element must have the declared scalar type.
    requiredParameters.compileSqlServerPlan({ parameters: { ids: [false], kind: 'preview' } })

    const query = batchQuery({ graph: child, key: { path: 'id', parameter: ids } })
    const relation = { name: 'children', from: 'id', query, cardinality: 'many' } as const
    batchRelation({ ...relation, parameters: { kind: 'preview' } })
    batchRelation({ ...relation, parameters: { kind: 'preview', labels: ['a', 'b'] as const } })
    // @ts-expect-error Excluding the batch key leaves kind required.
    batchRelation(relation)
    // @ts-expect-error Optional static parameters do not satisfy required ones.
    batchRelation({ ...relation, parameters: { labels: [] } })
    // @ts-expect-error The deferred batch key cannot be supplied as a static parameter.
    batchRelation({ ...relation, parameters: { kind: 'preview', ids: [1] } })
    // @ts-expect-error Static child parameters reject unknown names.
    batchRelation({ ...relation, parameters: { kind: 'preview', unknown: true } })

    const broadParameters = composeGraph({ root: broad, relations: [] })
    broad.compileSqlServer({})
    broad.compileSqlServer({ parameters: { dynamic: ['a', 1] } })
    broadParameters.compileSqlServerPlan({})
    broadParameters.compileSqlServerPlan({ parameters: { dynamic: ['a', 1] } })
  }

  t.is(typeof check, 'function')
})
