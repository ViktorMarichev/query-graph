import test from 'ava'
import { field, project as positionalProject, relation as positionalRelation } from '../definition.js'

import { registerDefinition } from '../index.js'
import {
  count,
  coalesce,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  dimension,
  eq,
  measure,
  nullable,
  project,
  relation,
  source,
  sum,
} from '../dsl.js'
import type { QueryOperation, ResultOf, ScalarOutputTypeMap } from '../dsl.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const attachment = source('attachment', {
  id: 'int64',
  description: nullable('string'),
  createdAt: 'dateTime',
})

const link = source('link', {
  id: 'int64',
  idAttachment: 'int64',
  idStorage: 'int64',
})

const storage = source('storage', {
  id: 'int64',
  name: 'string',
  size: nullable('string'),
})

const storageModule = defineGraphModule({
  name: 'storageDetails',
  sources: [link, storage],
  relations: [
    relation({
      name: 'storage',
      from: link,
      to: storage,
      required: true,
      on: eq(link.field('idStorage'), storage.field('id')),
    }),
  ],
  projection: [
    project({ path: 'links.idStorage', expression: link.field('idStorage') }),
    project({ path: 'links.storage.name', expression: storage.field('name') }),
    project({ path: 'links.storage.size', expression: storage.field('size') }),
    project({
      path: 'links.storage.displayName',
      expression: coalesce(storage.field('name'), 'Unavailable'),
    }),
  ],
})

const definition = defineGraph({
  name: 'attachments',
  root: attachment,
  modules: [storageModule],
  sources: [attachment],
  relations: [
    relation({
      name: 'links',
      from: attachment,
      to: link,
      cardinality: 'many',
      on: eq(attachment.field('id'), link.field('idAttachment')),
    }),
  ],
  projection: [
    project({ path: 'idAttachment', expression: attachment.field('id'), default: true }),
    project({ path: 'description', expression: attachment.field('description'), default: true }),
    project({ path: 'createdAt', expression: attachment.field('createdAt') }),
  ],
})

test('infers default projection result types', (t) => {
  type Actual = ResultOf<typeof definition>
  type Expected = {
    idAttachment: number | string
    description: string | null
  }

  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)
})

test('infers explicit nested selections and relation nullability', (t) => {
  const operation = {
    select: [
      'idAttachment',
      'createdAt',
      'links.idStorage',
      'links.storage.name',
      'links.storage.size',
      'links.storage.displayName',
    ],
  } as const satisfies QueryOperation<typeof definition>

  type Actual = ResultOf<typeof definition, typeof operation>
  type Expected = {
    idAttachment: number | string
    createdAt: string
    links: {
      idStorage: number | string | null
      storage: {
        name: string | null
        size: string | null
        displayName: string
      }
    }
  }

  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)

  const graph = registerDefinition(definition)
  type RegisteredResult = ResultOf<typeof graph, typeof operation>
  const registeredGraphKeepsDefinition: Equal<RegisteredResult, Expected> = true
  t.true(registeredGraphKeepsDefinition)

  const relationalGraph = graph.withRelationalMapping({
    sources: {
      attachment: { table: 'Attachment' },
      link: { table: 'AttachmentStorageLink' },
      storage: { table: 'Storage' },
    },
  })
  type RelationalResult = ResultOf<typeof relationalGraph, typeof operation>
  const relationalGraphKeepsDefinition: Equal<RelationalResult, Expected> = true
  t.true(relationalGraphKeepsDefinition)
})

test('supports driver-specific scalar output types', (t) => {
  interface DriverScalarTypes extends ScalarOutputTypeMap {
    boolean: boolean
    int32: number
    int64: bigint
    float64: number
    decimal: string
    string: string
    date: Date
    dateTime: Date
    binary: Uint8Array
    json: unknown
  }

  const operation = {
    select: ['idAttachment', 'createdAt', 'links.idStorage'],
  } as const satisfies QueryOperation<typeof definition>

  type Actual = ResultOf<typeof definition, typeof operation, DriverScalarTypes>
  type Expected = {
    idAttachment: bigint
    createdAt: Date
    links: {
      idStorage: bigint | null
    }
  }

  const driverTypesAreApplied: Equal<Actual, Expected> = true
  t.true(driverTypesAreApplied)
})

test('infers result types through the positional DSL', (t) => {
  const account = source('account', { id: 'int64' })
  const profile = source('profile', {
    idAccount: 'int64',
    displayName: nullable('string'),
  })

  const positionalDefinition = defineGraph({
    name: 'accounts',
    root: account,
    sources: [account, profile],
    relations: [positionalRelation('profile', account, profile, eq(field(account, 'id'), field(profile, 'idAccount')))],
    projection: [
      positionalProject('id', field(account, 'id'), { default: true }),
      positionalProject('profile.displayName', field(profile, 'displayName'), { default: true }),
    ],
  })

  type Actual = ResultOf<typeof positionalDefinition>
  type Expected = {
    id: number | string
    profile: {
      displayName: string | null
    }
  }

  const positionalTypesAreInferred: Equal<Actual, Expected> = true
  t.true(positionalTypesAreInferred)
})

test('infers summary dimensions and measures', (t) => {
  const invoice = source('invoice', {
    id: 'int64',
    idCustomer: 'int64',
    total: 'decimal',
  })

  const summaryDefinition = defineSummaryGraph({
    name: 'invoiceSummary',
    root: invoice,
    sources: [invoice],
    dimensions: [dimension({ path: 'customerId', expression: invoice.field('idCustomer'), default: true })],
    measures: [
      measure({ path: 'invoiceCount', expression: count(invoice.field('id')), default: true }),
      measure({ path: 'total', expression: sum(invoice.field('total')), default: true }),
    ],
  })

  type Actual = ResultOf<typeof summaryDefinition>
  type Expected = {
    customerId: number | string
    invoiceCount: number | string
    total: number | string | null
  }

  const summaryTypesAreInferred: Equal<Actual, Expected> = true
  t.true(summaryTypesAreInferred)
})
