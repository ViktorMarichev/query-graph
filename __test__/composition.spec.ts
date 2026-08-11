import test from 'ava'

import {
  asc,
  concat,
  constraint,
  defineGraph,
  eq,
  inParameter,
  ordering,
  param,
  project,
  requiredListParameter,
  requiredParameter,
  source,
} from '../definition.js'
import type { QueryGraphError, ResultOf, SqlServerCompileOptions } from '../definition.js'
import { batchRelation, composeGraph, registerDefinition } from '../index.js'

const news = source('news', {
  id: 'int64',
  idAttachment: 'int64',
  idOrganisation: 'int64',
  profileName: 'string',
})
const idOrganisation = requiredParameter('idOrganisation', 'int64')
const newsDefinition = defineGraph({
  name: 'news',
  root: news,
  sources: [news],
  parameters: [idOrganisation],
  constraints: [constraint(eq(news.field('idOrganisation'), param(idOrganisation)))],
  projection: [
    project('id', news.field('id'), { default: true }),
    project('idAttachment', news.field('idAttachment')),
    project('profile.name', news.field('profileName')),
  ],
})
const newsGraph = registerDefinition(newsDefinition).withRelationalMapping({
  sources: { news: { table: 'News' } },
})

const attachment = source('attachment', {
  idAttachment: 'int64',
  path: 'string',
  kind: 'string',
})
const attachmentIds = requiredListParameter('attachmentIds', 'int64')
const attachmentKind = requiredParameter('attachmentKind', 'string')
const attachmentDefinition = defineGraph({
  name: 'attachmentsByIds',
  root: attachment,
  sources: [attachment],
  parameters: [attachmentIds, attachmentKind],
  constraints: [
    constraint(inParameter(attachment.field('idAttachment'), attachmentIds)),
    constraint(eq(attachment.field('kind'), param(attachmentKind))),
  ],
  projection: [
    project('idAttachment', attachment.field('idAttachment')),
    project('path', attachment.field('path')),
    project('display', concat(attachment.field('path'), ' preview')),
  ],
  orderings: [
    ordering({
      name: 'pathAsc',
      by: [asc(attachment.field('path'))],
    }),
  ],
})
const attachmentGraph = registerDefinition(attachmentDefinition).withRelationalMapping({
  sources: { attachment: { table: 'Attachment' } },
})

const previewRelation = batchRelation({
  name: 'preview',
  from: 'idAttachment',
  graph: attachmentGraph,
  to: 'idAttachment',
  parameter: attachmentIds,
  cardinality: 'one',
  parameters: { attachmentKind: 'preview' },
  ordering: 'pathAsc',
})
const badgeRelation = batchRelation({
  name: 'badge',
  from: 'idAttachment',
  graph: attachmentGraph,
  to: 'idAttachment',
  parameter: 'attachmentIds',
  cardinality: 'many',
  parameters: { attachmentKind: 'badge' },
})
const graph = composeGraph({
  root: newsGraph,
  relations: [previewRelation, badgeRelation],
})

test('keeps dotted root projections and compiles selected batches lazily', (t) => {
  const options: SqlServerCompileOptions = { version: '2008' }
  const plan = graph.compileSqlServerPlan(
    {
      select: ['profile.name', 'preview.display', 'badge.path'],
      parameters: { idOrganisation: 7 },
    },
    options,
  )
  options.version = '2022'

  t.deepEqual(
    plan.root.columns.map((column) => column.path),
    ['profile.name', 'idAttachment'],
  )
  t.false(plan.root.sql.includes('FROM [Attachment]'))
  t.deepEqual(plan.batches, [
    {
      name: 'preview',
      parentKey: 'idAttachment',
      childKey: 'idAttachment',
      cardinality: 'one',
      keyParameter: 'attachmentIds',
      parameters: { attachmentKind: 'preview' },
      parentKeyInjected: true,
      childKeyInjected: true,
    },
    {
      name: 'badge',
      parentKey: 'idAttachment',
      childKey: 'idAttachment',
      cardinality: 'many',
      keyParameter: 'attachmentIds',
      parameters: { attachmentKind: 'badge' },
      parentKeyInjected: true,
      childKeyInjected: true,
    },
  ])

  const preview = plan.compileBatch('preview', [12, 18])
  t.regex(preview.sql, /FROM \[Attachment\]/)
  t.regex(preview.sql, /COALESCE\(\[t0\]\.\[path\], N''\) \+/)
  t.false(preview.sql.includes('CONCAT('))
  t.deepEqual(
    preview.columns.map((column) => column.path),
    ['display', 'idAttachment'],
  )
  t.true(preview.bindings.some((binding) => binding.parameter === 'attachmentIds' && binding.index === 1))
  t.true(preview.bindings.some((binding) => binding.parameter === 'attachmentKind'))
})

test('reports composition and deferred key errors through QueryGraphError', (t) => {
  const invalidStaticRelation = batchRelation({
    name: 'preview',
    from: 'idAttachment',
    graph: attachmentGraph,
    to: 'idAttachment',
    parameter: attachmentIds,
    cardinality: 'one',
    parameters: { attachmentKind: 42 as unknown as string },
  })

  const compositionError = t.throws(() =>
    composeGraph({
      root: newsGraph,
      relations: [invalidStaticRelation],
    }),
  ) as QueryGraphError
  t.is(compositionError.code, 'QUERY_GRAPH_COMPOSITION_INVALID')
  t.is(compositionError.phase, 'composition')
  t.true(compositionError.issues.some((issue) => issue.code === 'invalidStaticParameterType'))

  const plan = graph.compileSqlServerPlan({
    select: ['preview.path'],
    parameters: { idOrganisation: 7 },
  })
  const keyError = t.throws(() => plan.compileBatch('preview', [true] as never)) as QueryGraphError
  t.is(keyError.code, 'QUERY_GRAPH_OPERATION_INVALID')
  t.is(keyError.phase, 'operation')
  t.like(keyError.issues[0], {
    code: 'invalidParameterType',
    location: 'parameters.attachmentIds[0]',
  })
})

test('rejects unknown batch relation configuration keys', (t) => {
  const error = t.throws(() =>
    batchRelation({
      name: 'preview',
      from: 'idAttachment',
      graph: attachmentGraph,
      to: 'idAttachment',
      parameter: attachmentIds,
      cardinallity: 'one',
    } as never),
  )

  t.regex(error.message, /unknown configuration field "cardinallity"/)
})

test('preserves composition contracts in TypeScript', (t) => {
  const operation = {
    select: ['profile.name', 'preview.path'] as const,
    parameters: { idOrganisation: 7 },
  }
  type Row = ResultOf<typeof graph, typeof operation>
  const row: Row = {
    profile: { name: 'Article' },
    preview: { path: '/preview.jpg' },
  }
  t.deepEqual(row, {
    profile: { name: 'Article' },
    preview: { path: '/preview.jpg' },
  })

  const invalidCalls = () => {
    // @ts-expect-error Required root parameters remain required after composition.
    graph.compileSqlServerPlan({ select: ['preview.path'] })

    // @ts-expect-error Batch keys use the scalar type of the child list parameter.
    graph.compileSqlServerPlan(operation).compileBatch('preview', [true])

    // @ts-expect-error Only selected relation names can be compiled.
    graph.compileSqlServerPlan(operation).compileBatch('missing', [12])

    const invalidFrom = batchRelation({
      name: 'invalidFrom',
      from: 'missing',
      graph: attachmentGraph,
      to: 'idAttachment',
      parameter: attachmentIds,
      cardinality: 'one',
      parameters: { attachmentKind: 'preview' },
    })
    // @ts-expect-error Parent keys must be root projection paths.
    composeGraph({ root: newsGraph, relations: [invalidFrom] })

    batchRelation({
      name: 'invalidParameter',
      from: 'idAttachment',
      graph: attachmentGraph,
      to: 'idAttachment',
      // @ts-expect-error The key parameter must have list shape.
      parameter: attachmentKind,
      cardinality: 'one',
      parameters: { attachmentIds: [], attachmentKind: 'preview' },
    })

    batchRelation({
      name: 'invalidStaticValue',
      from: 'idAttachment',
      graph: attachmentGraph,
      to: 'idAttachment',
      parameter: attachmentIds,
      cardinality: 'one',
      // @ts-expect-error Static child parameters preserve their scalar types.
      parameters: { attachmentKind: 42 },
    })

    const invalidRow: Row = {
      profile: { name: 'Article' },
      // @ts-expect-error A to-one batch relation is an object or null, not an array.
      preview: [],
    }
    return invalidRow
  }

  t.is(typeof invalidCalls, 'function')
})
