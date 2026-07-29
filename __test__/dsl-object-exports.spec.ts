import test from 'ava'

import * as canonicalObjectDsl from '../dsl.js'
import * as functionalDsl from '../definition.js'
import * as objectDsl from '@query-graph/dsl-object'

const sharedPrimitives = ['source', 'eq', 'count', 'asc', 'ordering', 'firstBy', 'defineGraph'] as const
const structuralFactories = ['relation', 'constraint', 'project', 'dimension', 'measure'] as const

test('object DSL package is a thin facade over the canonical object API', (t) => {
  t.deepEqual(Object.keys(objectDsl).sort(), Object.keys(canonicalObjectDsl).sort())

  for (const name of Object.keys(canonicalObjectDsl) as Array<keyof typeof canonicalObjectDsl>) {
    const packageExport = objectDsl[name as keyof typeof objectDsl]
    t.is(packageExport as unknown, canonicalObjectDsl[name] as unknown)
  }
})

test('positional DSL is a compatibility adapter over shared object primitives', (t) => {
  for (const name of sharedPrimitives) {
    t.is(canonicalObjectDsl[name], functionalDsl[name])
  }

  for (const name of structuralFactories) {
    t.not(canonicalObjectDsl[name], functionalDsl[name])
  }

  t.false('GRAPH_DEFINITION_VERSION' in canonicalObjectDsl)
  t.false('field' in canonicalObjectDsl)
  t.false('GRAPH_DEFINITION_VERSION' in objectDsl)
  t.false('field' in objectDsl)
})

test('object DSL package and canonical type surfaces stay aligned', (t) => {
  type MissingExport = Exclude<keyof typeof canonicalObjectDsl, keyof typeof objectDsl>
  type ExtraExport = Exclude<keyof typeof objectDsl, keyof typeof canonicalObjectDsl>

  const hasMissingExports: MissingExport extends never ? false : true = false
  const hasExtraExports: ExtraExport extends never ? false : true = false

  t.false(hasMissingExports)
  t.false(hasExtraExports)
})
