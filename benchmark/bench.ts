import { Bench } from 'tinybench'

import { registerDefinition } from '../index.js'

const attributeValueGraphDefinition = {
  schemaVersion: 1,
  name: 'controllerAttributeValues',
  root: 'link',

  sources: [
    {
      key: 'link',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'idOwner', scalarType: 'int64' },
        { name: 'idControllerObjectValue', scalarType: 'int64' },
        { name: 'idOrganisation', scalarType: 'int64' },
        { name: 'order', scalarType: 'int32' },
        { name: 'dateDelete', scalarType: 'dateTime', nullable: true },
      ],
    },
    {
      key: 'value',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'idControllerObjectRequisite', scalarType: 'int64' },
        { name: 'idAttachment', scalarType: 'int64', nullable: true },
        { name: 'value', scalarType: 'string', nullable: true },
        { name: 'order', scalarType: 'int32' },
      ],
    },
    {
      key: 'requisite',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'idType', scalarType: 'int64' },
        { name: 'code', scalarType: 'string' },
        { name: 'name', scalarType: 'string' },
      ],
    },
    {
      key: 'requisiteType',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'code', scalarType: 'string' },
      ],
    },
    {
      key: 'attachment',
      fields: [
        { name: 'idAttachment', scalarType: 'int64' },
        { name: 'idStorage', scalarType: 'int64' },
      ],
    },
    {
      key: 'storage',
      fields: [{ name: 'id', scalarType: 'int64' }],
    },
  ],

  parameters: [
    { name: 'idOwner', scalarType: 'int64', required: true },
    { name: 'idOrganisation', scalarType: 'int64', required: true },
  ],

  relations: [
    {
      name: 'value',
      from: 'link',
      to: 'value',
      required: true,
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idControllerObjectValue' },
        right: { kind: 'field', source: 'value', field: 'id' },
      },
    },
    {
      name: 'requisite',
      from: 'value',
      to: 'requisite',
      required: true,
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'value', field: 'idControllerObjectRequisite' },
        right: { kind: 'field', source: 'requisite', field: 'id' },
      },
    },
    {
      name: 'requisiteType',
      from: 'requisite',
      to: 'requisiteType',
      required: true,
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'requisite', field: 'idType' },
        right: { kind: 'field', source: 'requisiteType', field: 'id' },
      },
    },
    {
      name: 'attachment',
      from: 'value',
      to: 'attachment',
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'value', field: 'idAttachment' },
        right: { kind: 'field', source: 'attachment', field: 'idAttachment' },
      },
    },
    {
      name: 'storage',
      from: 'attachment',
      to: 'storage',
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'attachment', field: 'idStorage' },
        right: { kind: 'field', source: 'storage', field: 'id' },
      },
    },
  ],

  constraints: [
    {
      name: 'owner',
      predicate: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idOwner' },
        right: { kind: 'parameter', name: 'idOwner' },
      },
    },
    {
      name: 'organisation',
      predicate: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idOrganisation' },
        right: { kind: 'parameter', name: 'idOrganisation' },
      },
    },
    {
      name: 'active',
      predicate: {
        kind: 'isNull',
        expression: { kind: 'field', source: 'link', field: 'dateDelete' },
      },
    },
  ],

  projection: {
    fields: [
      {
        path: ['value', 'id'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'id' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'value'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'value' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'requisite', 'code'],
        relations: ['value', 'requisite'],
        expression: { kind: 'field', source: 'requisite', field: 'code' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'requisite', 'name'],
        relations: ['value', 'requisite'],
        expression: { kind: 'field', source: 'requisite', field: 'name' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'requisite', 'type'],
        relations: ['value', 'requisite', 'requisiteType'],
        expression: { kind: 'field', source: 'requisiteType', field: 'code' },
        selectedByDefault: true,
      },
      {
        path: ['value', 'file', 'url'],
        relations: ['value', 'attachment', 'storage'],
        expression: {
          kind: 'function',
          name: 'publicStorageUrl',
          arguments: [{ kind: 'field', source: 'storage', field: 'id' }],
        },
      },
    ],
  },

  defaultOrderBy: [
    {
      expression: { kind: 'field', source: 'link', field: 'order' },
      direction: 'asc',
    },
    {
      expression: { kind: 'field', source: 'value', field: 'order' },
      direction: 'asc',
    },
  ],
}

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

const bench = new Bench()

bench.add('Validate and index graph definition', () => {
  registerDefinition(attributeValueGraphDefinition)
})

bench.add('Compile SQL Server statement', () => {
  relationalGraph.compileSqlServer(operation)
})

await bench.run()

console.table(bench.table())
