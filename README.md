# query-graph

Native query graph planning and SQL compilation for Node.js.

The package owns graph validation, planning, and SQL generation. Database
connections and query execution remain the responsibility of the consumer.

## Current foundation

The first implemented layer is a dialect-neutral graph definition:

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
      fields: [{ name: 'id', scalarType: 'int64' }],
    },
  ],
  relations: [
    {
      name: 'value',
      from: 'link',
      to: 'value',
      on: {
        kind: 'eq',
        left: { kind: 'field', source: 'link', field: 'idControllerObjectValue' },
        right: { kind: 'field', source: 'value', field: 'id' },
      },
    },
  ],
})
```

The definition contains no SQL text, table names, driver values, or executable
JavaScript callbacks. Future layers will add operations, relational mappings,
and Oracle/SQL Server compilers on top of the same native graph handle.
