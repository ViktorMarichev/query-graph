import test from 'ava'

import * as functionalDsl from '../definition.js'
import * as objectDsl from '@query-graph/dsl-object'

test('object DSL exposes the complete functional DSL surface', (t) => {
  t.deepEqual(Object.keys(objectDsl).sort(), Object.keys(functionalDsl).sort())
})
