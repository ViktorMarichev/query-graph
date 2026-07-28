import test from 'ava'

import * as functionalDsl from '../definition.js'
import * as objectDsl from '@query-graph/dsl-object'

const sharedPrimitives = ['source', 'eq', 'count', 'asc', 'firstBy', 'defineGraph'] as const
const structuralFactories = ['relation', 'constraint', 'project', 'dimension', 'measure'] as const

test('object DSL exposes a deliberate authoring surface', (t) => {
  for (const name of sharedPrimitives) {
    t.is(objectDsl[name], functionalDsl[name])
  }

  for (const name of structuralFactories) {
    t.not(objectDsl[name], functionalDsl[name])
  }

  t.false('GRAPH_DEFINITION_VERSION' in objectDsl)
  t.false('field' in objectDsl)
})

test('object DSL type surface excludes wire and duplicate field helpers', (t) => {
  type ObjectDslExport = keyof typeof objectDsl
  type HasWireVersion = 'GRAPH_DEFINITION_VERSION' extends ObjectDslExport ? true : false
  type HasStandaloneField = 'field' extends ObjectDslExport ? true : false

  const hasWireVersion: HasWireVersion = false
  const hasStandaloneField: HasStandaloneField = false

  t.false(hasWireVersion)
  t.false(hasStandaloneField)
})
