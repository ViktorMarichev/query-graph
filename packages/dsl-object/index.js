'use strict'

const functional = require('query-graph/definition')

const RELATION_CONFIGURATION_KEYS = new Set([
  'name',
  'from',
  'to',
  'on',
  'required',
  'cardinality',
  'selection',
])
const CONSTRAINT_CONFIGURATION_KEYS = new Set(['name', 'predicate', 'when'])
const PROJECTION_CONFIGURATION_KEYS = new Set(['path', 'expression', 'default'])

function configurationOf(factory, value, allowedKeys) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${factory} expects a configuration object`)
  }

  const unknownKeys = Reflect.ownKeys(value).filter(
    (key) => typeof key !== 'string' || !allowedKeys.has(key),
  )
  if (unknownKeys.length > 0) {
    const label = unknownKeys.length === 1 ? 'field' : 'fields'
    const keys = unknownKeys.map((key) => JSON.stringify(String(key))).join(', ')
    throw new TypeError(`${factory} received unknown configuration ${label} ${keys}`)
  }

  return value
}

function relation(value) {
  const configuration = configurationOf('relation', value, RELATION_CONFIGURATION_KEYS)

  return functional.relation(configuration.name, configuration.from, configuration.to, configuration.on, {
    required: configuration.required,
    cardinality: configuration.cardinality,
    selection: configuration.selection,
  })
}

function constraint(value) {
  const configuration = configurationOf('constraint', value, CONSTRAINT_CONFIGURATION_KEYS)

  return functional.constraint(configuration.name, configuration.predicate, {
    when: configuration.when,
  })
}

function project(value) {
  const configuration = configurationOf('project', value, PROJECTION_CONFIGURATION_KEYS)

  return functional.project(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

function dimension(value) {
  const configuration = configurationOf('dimension', value, PROJECTION_CONFIGURATION_KEYS)

  return functional.dimension(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

function measure(value) {
  const configuration = configurationOf('measure', value, PROJECTION_CONFIGURATION_KEYS)

  return functional.measure(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

exports.fieldType = functional.fieldType
exports.nullable = functional.nullable
exports.hidden = functional.hidden
exports.source = functional.source
exports.requiredParameter = functional.requiredParameter
exports.optionalParameter = functional.optionalParameter
exports.param = functional.param
exports.literal = functional.literal
exports.integer = functional.integer
exports.decimal = functional.decimal
exports.requiredListParameter = functional.requiredListParameter
exports.optionalListParameter = functional.optionalListParameter
exports.eq = functional.eq
exports.neq = functional.neq
exports.lt = functional.lt
exports.lte = functional.lte
exports.gt = functional.gt
exports.gte = functional.gte
exports.like = functional.like
exports.inList = functional.inList
exports.and = functional.and
exports.or = functional.or
exports.not = functional.not
exports.isNull = functional.isNull
exports.isNotNull = functional.isNotNull
exports.inParameter = functional.inParameter
exports.exists = functional.exists
exports.lower = functional.lower
exports.upper = functional.upper
exports.coalesce = functional.coalesce
exports.concat = functional.concat
exports.count = functional.count
exports.countDistinct = functional.countDistinct
exports.sum = functional.sum
exports.average = functional.average
exports.minimum = functional.minimum
exports.maximum = functional.maximum
exports.relation = relation
exports.constraint = constraint
exports.project = project
exports.dimension = dimension
exports.measure = measure
exports.asc = functional.asc
exports.desc = functional.desc
exports.defineGraphModule = functional.defineGraphModule
exports.defineGraph = functional.defineGraph
exports.defineSummaryGraph = functional.defineSummaryGraph
exports.firstBy = functional.firstBy
