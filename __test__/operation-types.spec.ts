import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  asc,
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
  param,
  project,
  relation,
  requiredParameter,
  source,
} from '../definition.js'

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
    constraints: [constraint('organisation', eq(staff.field('idOrganisation'), param(idOrganisation)))],
    projection: [project('id', staff.field('id'), { default: true })],
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
      constraint('search', like(staff.field('name'), param(search)), { when: search }),
      constraint('personIds', exists(personStaff, inParameter(personStaff.field('idPerson'), personIds)), {
        when: personIds,
      }),
    ],
    projection: [
      project(['credentials', 'idPerson'], personStaff.field('idPerson'), {
        default: true,
      }),
    ],
  })

  const graph = registerDefinition(definition)
  const relationalGraph = graph.withRelationalMapping({
    sources: {
      staff: { table: 'Staff' },
      personStaff: { table: 'PersonStaff' },
    },
  })

  relationalGraph.compileSqlServer({
    select: ['id', 'credentials.idPerson'],
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
