import { Bench } from 'tinybench'

import {
  asc,
  coalesce,
  constraint,
  defineGraph,
  eq,
  isNull,
  nullable,
  ordering,
  param,
  project,
  relation,
  requiredParameter,
  source,
} from '../dsl.js'
import { registerDefinition } from '../index.js'

const link = source('link', {
  id: 'int64',
  idOwner: 'int64',
  idControllerObjectValue: 'int64',
  idOrganisation: 'int64',
  order: 'int32',
  dateDelete: nullable('dateTime'),
})

const value = source('value', {
  id: 'int64',
  idControllerObjectRequisite: 'int64',
  idAttachment: nullable('int64'),
  value: nullable('string'),
  order: 'int32',
})

const requisite = source('requisite', {
  id: 'int64',
  idType: 'int64',
  code: 'string',
  name: 'string',
})

const requisiteType = source('requisiteType', {
  id: 'int64',
  code: 'string',
})

const attachment = source('attachment', {
  idAttachment: 'int64',
  idStorage: 'int64',
})

const storage = source('storage', {
  id: 'int64',
})

const idOwner = requiredParameter('idOwner', 'int64')
const idOrganisation = requiredParameter('idOrganisation', 'int64')

const valueRelation = relation({
  name: 'value',
  from: link,
  to: value,
  on: eq(link.field('idControllerObjectValue'), value.field('id')),
  required: true,
})

const requisiteRelation = relation({
  name: 'requisite',
  from: value,
  to: requisite,
  on: eq(value.field('idControllerObjectRequisite'), requisite.field('id')),
  required: true,
})

const requisiteTypeRelation = relation({
  name: 'requisiteType',
  from: requisite,
  to: requisiteType,
  on: eq(requisite.field('idType'), requisiteType.field('id')),
  required: true,
})

const attachmentRelation = relation({
  name: 'attachment',
  from: value,
  to: attachment,
  on: eq(value.field('idAttachment'), attachment.field('idAttachment')),
})

const storageRelation = relation({
  name: 'storage',
  from: attachment,
  to: storage,
  on: eq(attachment.field('idStorage'), storage.field('id')),
})

const attributeValueGraphDefinition = defineGraph({
  name: 'controllerAttributeValues',
  root: link,
  sources: [link, value, requisite, requisiteType, attachment, storage],
  parameters: [idOwner, idOrganisation],
  relations: [valueRelation, requisiteRelation, requisiteTypeRelation, attachmentRelation, storageRelation],
  constraints: [
    constraint({ predicate: eq(link.field('idOwner'), param(idOwner)) }),
    constraint({ predicate: eq(link.field('idOrganisation'), param(idOrganisation)) }),
    constraint({ predicate: isNull(link.field('dateDelete')) }),
  ],
  projection: [
    project({
      path: 'value.id',
      expression: value.field('id'),
      default: true,
    }),
    project({
      path: 'value.value',
      expression: value.field('value'),
      default: true,
    }),
    project({
      path: 'value.requisite.code',
      expression: requisite.field('code'),
      default: true,
    }),
    project({
      path: 'value.requisite.name',
      expression: requisite.field('name'),
      default: true,
    }),
    project({
      path: 'value.requisite.type',
      expression: requisiteType.field('code'),
      default: true,
    }),
    project({ path: 'value.file.storageId', expression: coalesce(storage.field('id'), 0) }),
  ],
  orderings: [
    ordering({
      name: 'default',
      by: [asc(link.field('order')), asc(value.field('order'))],
      default: true,
    }),
  ],
})

const relationalMapping = {
  sources: {
    link: {
      table: {
        schema: 'dbo',
        name: 'BUSINESS-OBJECT#ControllerAttributeValueLink',
      },
    },
    value: {
      table: {
        schema: 'dbo',
        name: 'SOFTWARE#ControllerObjectValue',
      },
    },
    requisite: {
      table: {
        schema: 'dbo',
        name: 'BUSINESS-OBJECT#ControllerObjectRequisite',
      },
    },
    requisiteType: {
      table: {
        schema: 'dbo',
        name: 'CORE#Reference',
      },
    },
    attachment: {
      table: {
        schema: 'dbo',
        name: 'ATTACHMENT#AttachmentStorage',
      },
    },
    storage: {
      table: {
        schema: 'dbo',
        name: 'STORAGE#Storage',
      },
    },
  },
}

const operation = {
  select: ['value.id', 'value.value', 'value.requisite.code', 'value.requisite.name', 'value.requisite.type'],
  parameters: {
    idOwner: 42,
    idOrganisation: 1,
  },
  offset: 0,
  limit: 25,
}

const graph = registerDefinition(attributeValueGraphDefinition)
const relationalGraph = graph.withRelationalMapping(relationalMapping)
const statement = relationalGraph.compileSqlServer(operation)

console.log(statement.sql)
console.table(statement.bindings)
console.table(statement.columns)
console.table(statement.relations)

const bench = new Bench()

bench.add('Validate and index graph definition', () => {
  registerDefinition(attributeValueGraphDefinition)
})

bench.add('Compile SQL Server statement', () => {
  relationalGraph.compileSqlServer(operation)
})

await bench.run()

console.table(bench.table())
