import test from 'ava'

import * as canonicalObjectDsl from '../dsl.js'
import * as functionalDsl from '../definition.js'
import * as objectDsl from '@query-graph/dsl-object'
import * as runtime from '../index.js'

const sharedPrimitives = [
  'source',
  'requiredParam',
  'optionalParam',
  'requiredListParam',
  'optionalListParam',
  'requiredGroupsParameter',
  'requiredGroupsParam',
  'optionalGroupsParameter',
  'optionalGroupsParam',
  'allGroups',
  'inParam',
  'eq',
  'count',
  'asc',
  'ordering',
  'firstBy',
  'defineGraph',
  'batchQuery',
  'batchRelation',
  'composeGraph',
] as const
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

  t.is(canonicalObjectDsl.requiredParam, canonicalObjectDsl.requiredParameter)
  t.is(canonicalObjectDsl.optionalParam, canonicalObjectDsl.optionalParameter)
  t.is(canonicalObjectDsl.requiredListParam, canonicalObjectDsl.requiredListParameter)
  t.is(canonicalObjectDsl.optionalListParam, canonicalObjectDsl.optionalListParameter)
  t.is(canonicalObjectDsl.requiredGroupsParam, canonicalObjectDsl.requiredGroupsParameter)
  t.is(canonicalObjectDsl.optionalGroupsParam, canonicalObjectDsl.optionalGroupsParameter)
  t.is(canonicalObjectDsl.inParam, canonicalObjectDsl.inParameter)

  t.false('GRAPH_DEFINITION_VERSION' in canonicalObjectDsl)
  t.false('field' in canonicalObjectDsl)
  t.false('GRAPH_DEFINITION_VERSION' in objectDsl)
  t.false('field' in objectDsl)
  for (const name of [
    'allGroups',
    'optionalGroupsParameter',
    'optionalGroupsParam',
    'requiredGroupsParameter',
    'requiredGroupsParam',
  ]) {
    t.false(name in runtime)
  }
})

test('group parameter factories do not retain the old export names', (t) => {
  const removedNames = [
    'optionalGroupedListParam',
    'optionalGroupedListParameter',
    'requiredGroupedListParam',
    'requiredGroupedListParameter',
  ] as const
  type RemainingExport = Extract<
    (typeof removedNames)[number],
    keyof typeof canonicalObjectDsl | keyof typeof functionalDsl | keyof typeof objectDsl | keyof typeof runtime
  >
  const hasRemainingExports: RemainingExport extends never ? false : true = false

  t.false(hasRemainingExports)
  for (const entrypoint of [canonicalObjectDsl, functionalDsl, objectDsl, runtime]) {
    for (const name of removedNames) {
      t.false(name in entrypoint)
    }
  }
})

test('object DSL package and canonical type surfaces stay aligned', (t) => {
  type MissingExport = Exclude<keyof typeof canonicalObjectDsl, keyof typeof objectDsl>
  type ExtraExport = Exclude<keyof typeof objectDsl, keyof typeof canonicalObjectDsl>

  const hasMissingExports: MissingExport extends never ? false : true = false
  const hasExtraExports: ExtraExport extends never ? false : true = false

  t.false(hasMissingExports)
  t.false(hasExtraExports)
})
