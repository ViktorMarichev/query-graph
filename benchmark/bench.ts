import { Bench } from 'tinybench'

import {
  asc,
  call,
  constraint,
  defineGraph,
  eq,
  isNull,
  nullable,
  param,
  project,
  relation,
  requiredParameter,
  source,
} from '../definition.js'
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

const valueRelation = relation('value', link, value, eq(link.field('idControllerObjectValue'), value.field('id')), {
  required: true,
})

const requisiteRelation = relation(
  'requisite',
  value,
  requisite,
  eq(value.field('idControllerObjectRequisite'), requisite.field('id')),
  { required: true },
)

const requisiteTypeRelation = relation(
  'requisiteType',
  requisite,
  requisiteType,
  eq(requisite.field('idType'), requisiteType.field('id')),
  { required: true },
)

const attachmentRelation = relation(
  'attachment',
  value,
  attachment,
  eq(value.field('idAttachment'), attachment.field('idAttachment')),
)

const storageRelation = relation('storage', attachment, storage, eq(attachment.field('idStorage'), storage.field('id')))

const attributeValueGraphDefinition = defineGraph({
  name: 'controllerAttributeValues',
  root: link,
  sources: [link, value, requisite, requisiteType, attachment, storage],
  parameters: [idOwner, idOrganisation],
  relations: [valueRelation, requisiteRelation, requisiteTypeRelation, attachmentRelation, storageRelation],
  constraints: [
    constraint('owner', eq(link.field('idOwner'), param(idOwner))),
    constraint('organisation', eq(link.field('idOrganisation'), param(idOrganisation))),
    constraint('active', isNull(link.field('dateDelete'))),
  ],
  projection: [
    project('value.id', value.field('id'), {
      through: [valueRelation],
      default: true,
    }),
    project('value.value', value.field('value'), {
      through: [valueRelation],
      default: true,
    }),
    project('value.requisite.code', requisite.field('code'), {
      through: [valueRelation, requisiteRelation],
      default: true,
    }),
    project('value.requisite.name', requisite.field('name'), {
      through: [valueRelation, requisiteRelation],
      default: true,
    }),
    project('value.requisite.type', requisiteType.field('code'), {
      through: [valueRelation, requisiteRelation, requisiteTypeRelation],
      default: true,
    }),
    project('value.file.url', call('publicStorageUrl', storage.field('id')), {
      through: [valueRelation, attachmentRelation, storageRelation],
    }),
  ],
  defaultOrderBy: [asc(link.field('order')), asc(value.field('order'))],
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
