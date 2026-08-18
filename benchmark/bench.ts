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
  idAttributeValue: 'int64',
  tenantId: 'int64',
  order: 'int32',
  dateDelete: nullable('dateTime'),
})

const value = source('value', {
  id: 'int64',
  idAttributeDefinition: 'int64',
  idAttachment: nullable('int64'),
  value: nullable('string'),
  order: 'int32',
})

const attribute = source('attribute', {
  id: 'int64',
  idType: 'int64',
  code: 'string',
  name: 'string',
})

const attributeType = source('attributeType', {
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
const tenantId = requiredParameter('tenantId', 'int64')

const valueRelation = relation({
  name: 'value',
  from: link,
  to: value,
  on: eq(link.field('idAttributeValue'), value.field('id')),
  required: true,
})

const attributeRelation = relation({
  name: 'attribute',
  from: value,
  to: attribute,
  on: eq(value.field('idAttributeDefinition'), attribute.field('id')),
  required: true,
})

const attributeTypeRelation = relation({
  name: 'attributeType',
  from: attribute,
  to: attributeType,
  on: eq(attribute.field('idType'), attributeType.field('id')),
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
  name: 'recordAttributeValues',
  root: link,
  sources: [link, value, attribute, attributeType, attachment, storage],
  parameters: [idOwner, tenantId],
  relations: [valueRelation, attributeRelation, attributeTypeRelation, attachmentRelation, storageRelation],
  constraints: [
    constraint({ predicate: eq(link.field('idOwner'), param(idOwner)) }),
    constraint({ predicate: eq(link.field('tenantId'), param(tenantId)) }),
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
      path: 'value.attribute.code',
      expression: attribute.field('code'),
      default: true,
    }),
    project({
      path: 'value.attribute.name',
      expression: attribute.field('name'),
      default: true,
    }),
    project({
      path: 'value.attribute.type',
      expression: attributeType.field('code'),
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
        name: 'APP#AttributeValueLink',
      },
    },
    value: {
      table: {
        schema: 'dbo',
        name: 'APP#AttributeValue',
      },
    },
    attribute: {
      table: {
        schema: 'dbo',
        name: 'APP#AttributeDefinition',
      },
    },
    attributeType: {
      table: {
        schema: 'dbo',
        name: 'APP#AttributeType',
      },
    },
    attachment: {
      table: {
        schema: 'dbo',
        name: 'APP#AssetLink',
      },
    },
    storage: {
      table: {
        schema: 'dbo',
        name: 'APP#StorageObject',
      },
    },
  },
}

const operation = {
  select: ['value.id', 'value.value', 'value.attribute.code', 'value.attribute.name', 'value.attribute.type'],
  parameters: {
    idOwner: 42,
    tenantId: 1,
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
