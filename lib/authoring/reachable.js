'use strict'

const { randomUUID } = require('node:crypto')
const { configurationOf, optionalArray, optionalEnum } = require('../configuration.js')
const { asExpression } = require('./primitives.js')
const { constraint, defineGraph } = require('./definition.js')

const KEYS = new Set(['source', 'modules', 'key', 'where', 'start', 'step', 'match', 'cycles'])

function reachable(value) {
  const configuration = configurationOf('reachable', value, KEYS)
  const { source, key, step } = configuration
  if (!source || typeof source.field !== 'function') {
    throw new TypeError('reachable source must be a source reference')
  }
  if (key?.kind !== 'field' || key.source !== source.key) {
    throw new TypeError('reachable key must be a field of its source')
  }
  if (typeof step !== 'function') throw new TypeError('reachable step must be a function')
  if (configuration.start === undefined || configuration.match === undefined) {
    throw new TypeError('reachable requires start and match predicates')
  }
  optionalArray(configuration.modules, 'reachable.modules')
  optionalEnum(configuration.cycles, new Set(['skip']), 'reachable cycles')

  const scope = randomUUID()
  const reference = (side) =>
    Object.freeze({
      field(name) {
        source.field(name)
        return { kind: 'traversalField', scope, side, field: name }
      },
    })
  const input = defineGraph({
    name: 'reachableInput',
    root: source,
    sources: [source],
    modules: configuration.modules,
    constraints: configuration.where === undefined ? [] : [constraint({ predicate: configuration.where })],
  })
  if (input.projection.fields.length || input.projection.objects.length || input.orderings.length) {
    throw new TypeError('reachable modules may not define projection, objects or orderings')
  }

  return {
    kind: 'reachable',
    scope,
    input,
    key: key.field,
    start: asExpression(configuration.start),
    step: asExpression(step({ current: reference('current'), next: reference('next') })),
    match: scopeMatch(asExpression(configuration.match), source.key, scope),
    cycles: configuration.cycles ?? 'skip',
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
