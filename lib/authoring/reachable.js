'use strict'

const { configurationOf } = require('../configuration.js')
const { asExpression } = require('./primitives.js')
const { TRAVERSAL_KEYS, buildTraversal } = require('./traversal.js')

const KEYS = new Set([...TRAVERSAL_KEYS, 'match'])

function reachable(value) {
  const configuration = configurationOf('reachable', value, KEYS)
  if (configuration.start === undefined || configuration.match === undefined) {
    throw new TypeError('reachable requires start and match predicates')
  }
  const traversal = buildTraversal(configuration, 'reachable')

  return {
    kind: 'reachable',
    ...traversal,
    match: scopeMatch(asExpression(configuration.match), configuration.source.key, traversal.scope),
  }
}

function scopeMatch(value, source, scope) {
  if (Array.isArray(value)) return value.map((item) => scopeMatch(item, source, scope))
  if (value === null || typeof value !== 'object') return value
  if (value.kind === 'field' && value.source === source) {
    return { kind: 'traversalField', scope, side: 'next', field: value.field }
  }
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, scopeMatch(item, source, scope)]))
}

module.exports = { reachable }
