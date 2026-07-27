# query-graph

Native query graph planning and SQL compilation for Node.js.

The package owns graph validation, planning, and SQL generation. Database
connections and query execution remain the responsibility of the consumer.

## Graph definition

The graph definition is dialect-neutral and contains:

- logical sources and fields;
- typed parameters;
- named relations;
- semantic constraints;
- output projection;
- default ordering.

`registerDefinition` transfers a definition to Rust once. Rust validates every
reference, creates reusable indexes, and returns a native `QueryGraph` handle.

```ts
import { registerDefinition } from 'query-graph'

const graph = registerDefinition({
  schemaVersion: 1,
  name: 'attributeValues',
  root: 'link',
  sources: [
    {
      key: 'link',
      fields: [
        { name: 'idOwner', scalarType: 'int64' },
        { name: 'idControllerObjectValue', scalarType: 'int64' },
      ],
    },
    {
      key: 'value',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'value', scalarType: 'string', nullable: true },
      ],
    },
  ],
  parameters: [{ name: 'idOwner', scalarType: 'int64', required: true }],
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
  ],
  projection: {
    fields: [
      {
        path: ['value', 'value'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'value' },
        selectedByDefault: true,
      },
    ],
  },
})
```

The definition contains no SQL text, table names, driver values, or executable
JavaScript callbacks.

## SQL Server

A relational mapping connects logical sources to physical tables. Logical field
names are used as column names by default; `columns` only contains overrides.

```ts
const relationalGraph = graph.withRelationalMapping({
  sources: {
    link: {
      table: {
        schema: 'dbo',
        name: 'ControllerAttributeValueLink',
      },
      columns: {
        idOwner: 'owner_id',
      },
    },
    value: {
      table: 'ControllerObjectValue',
    },
  },
})
```

An operation is sent to Rust as one call. The compiler chooses the required
relation paths, renders SQL Server syntax, and returns parameter descriptors.

```ts
const operation = {
  select: ['value.value'],
  parameters: {
    idOwner: 42,
  },
  offset: 0,
  limit: 25,
}

const statement = relationalGraph.compileSqlServer(operation)

console.log(statement.sql)
console.table(statement.bindings)
```

`statement.bindings` maps generated names such as `p0` back to logical
parameters such as `idOwner`. The consumer takes the values from
`operation.parameters` and passes them to its database driver.

The first SQL Server slice supports projection selection, definition
constraints, conditional constraints, default ordering, `INNER JOIN`/`LEFT
JOIN`, and `OFFSET`/`FETCH` pagination. Runtime filters and custom semantic
function mappings are intentionally left for subsequent layers.

See `benchmark/bench.ts` for a complete graph, mapping, operation, generated
SQL, and compilation benchmark.
