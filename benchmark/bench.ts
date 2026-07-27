import { Bench } from 'tinybench'

import { registerDefinition } from '../index.js'

const graph = registerDefinition({
  schemaVersion: 1,
  name: 'benchmark',
  root: 'link',
  sources: [
    {
      key: 'link',
      fields: [{ name: 'idControllerObjectValue', scalarType: 'int64' }],
    },
    {
      key: 'value',
      fields: [
        { name: 'id', scalarType: 'int64' },
        { name: 'value', scalarType: 'string', nullable: true },
      ],
    },
  ],
  parameters: [],
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
  constraints: [],
  projection: {
    fields: [
      {
        path: ['value'],
        relations: ['value'],
        expression: { kind: 'field', source: 'value', field: 'value' },
      },
    ],
  },
})

const fields = new Set(['link.idControllerObjectValue', 'value.id', 'value.value'])
const bench = new Bench()

bench.add('Native graph field lookup', () => {
  graph.hasField('value', 'value')
})

bench.add('JavaScript Set field lookup', () => {
  fields.has('value.value')
})

await bench.run()

console.table(bench.table())
