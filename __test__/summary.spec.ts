import test from 'ava'

import type { QueryGraphError } from '../definition.js'
import { registerDefinition } from '../index.js'
import {
  countDistinct,
  constraint,
  defineSummaryGraph,
  desc,
  dimension,
  eq,
  gte,
  measure,
  ordering,
  param,
  relation,
  requiredParameter,
  source,
  sum,
} from '../definition.js'

test('builds and compiles a typed summary graph', (t) => {
  const service = source('service', {
    id: 'int64',
    idOrganisation: 'int64',
  })
  const staff = source('staff', {
    idService: 'int64',
    idStaff: 'int64',
    hours: 'decimal',
  })
  const idOrganisation = requiredParameter('idOrganisation', 'int64')
  const minimumStaff = requiredParameter('minimumStaff', 'int64')
  const serviceId = dimension('serviceId', service.field('id'), { default: true })
  const staffCount = measure('staffCount', countDistinct(staff.field('idStaff')), {
    default: true,
  })

  const definition = defineSummaryGraph({
    name: 'serviceSummary',
    root: service,
    sources: [service, staff],
    parameters: [idOrganisation, minimumStaff],
    relations: [
      relation('staff', service, staff, eq(service.field('id'), staff.field('idService')), {
        cardinality: 'many',
      }),
    ],
    constraints: [
      constraint(eq(service.field('idOrganisation'), param(idOrganisation))),
      constraint(gte(staffCount, param(minimumStaff))),
    ],
    dimensions: [serviceId],
    measures: [
      staffCount,
      measure('totalHours', sum(staff.field('hours')), {
        default: true,
      }),
    ],
    orderings: [
      ordering({
        name: 'staffCountDesc',
        by: [desc(staffCount)],
        default: true,
      }),
    ],
  })

  t.is(definition.schemaVersion, 10)
  t.deepEqual(
    definition.projection.fields.map(({ path, role }) => [path.join('.'), role]),
    [
      ['serviceId', 'dimension'],
      ['staffCount', 'measure'],
      ['totalHours', 'measure'],
    ],
  )
  t.deepEqual(definition.constraints[1].predicate, {
    kind: 'greaterThanOrEqual',
    left: {
      kind: 'aggregate',
      function: 'countDistinct',
      expression: {
        kind: 'field',
        source: 'staff',
        field: 'idStaff',
      },
    },
    right: {
      kind: 'parameter',
      name: 'minimumStaff',
    },
  })

  const graph = registerDefinition(definition).withRelationalMapping({
    sources: {
      service: { table: 'Service' },
      staff: { table: 'ServiceStaff' },
    },
  })
  const statement = graph.compileSqlServer({
    select: ['serviceId', 'staffCount', 'totalHours'],
    parameters: {
      idOrganisation: 7,
      minimumStaff: 2,
    },
  })

  t.true(statement.sql.includes('WHERE\n  ([t0].[idOrganisation] = @p0)'))
  t.true(statement.sql.includes('GROUP BY\n  [t0].[id]'))
  t.true(statement.sql.includes('HAVING\n  (COUNT_BIG(DISTINCT [t1].[idStaff]) >= @p1)'))

  const tag = source('tag', {
    idService: 'int64',
    idTag: 'int64',
  })
  const fanoutDefinition = defineSummaryGraph({
    name: 'invalidServiceSummary',
    root: service,
    sources: [service, staff, tag],
    parameters: [idOrganisation, minimumStaff],
    relations: [
      relation('staff', service, staff, eq(service.field('id'), staff.field('idService')), {
        cardinality: 'many',
      }),
      relation('tags', service, tag, eq(service.field('id'), tag.field('idService')), {
        cardinality: 'many',
      }),
    ],
    constraints: [
      constraint(eq(service.field('idOrganisation'), param(idOrganisation))),
      constraint(gte(staffCount, param(minimumStaff))),
    ],
    dimensions: [serviceId],
    measures: [
      staffCount,
      measure('tagCount', countDistinct(tag.field('idTag')), {
        default: true,
      }),
    ],
  })
  const fanoutGraph = registerDefinition(fanoutDefinition).withRelationalMapping({
    sources: {
      service: { table: 'Service' },
      staff: { table: 'ServiceStaff' },
      tag: { table: 'ServiceTag' },
    },
  })
  const fanoutError = t.throws(() =>
    fanoutGraph.compileSqlServer({
      parameters: {
        idOrganisation: 7,
        minimumStaff: 2,
      },
    }),
  ) as QueryGraphError

  t.is(fanoutError.code, 'QUERY_GRAPH_SQL_COMPILE_FAILED')
  t.like(fanoutError.issues[0], {
    code: 'aggregationAcrossManyBranches',
    location: 'plan',
  })

  const invalidOperation = () => {
    graph.compileSqlServer({
      // @ts-expect-error Selection is restricted to summary output paths.
      select: ['unknown'],
      parameters: {
        idOrganisation: 7,
        minimumStaff: 2,
      },
    })
  }
  t.is(typeof invalidOperation, 'function')
})
