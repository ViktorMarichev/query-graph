'use strict'

const functional = require('query-graph/definition')

function configurationOf(factory, value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${factory} expects a configuration object`)
  }

  return value
}

function relation(value) {
  const configuration = configurationOf('relation', value)

  return functional.relation(configuration.name, configuration.from, configuration.to, configuration.on, {
    required: configuration.required,
    cardinality: configuration.cardinality,
    selection: configuration.selection,
  })
}

function constraint(value) {
  const configuration = configurationOf('constraint', value)

  return functional.constraint(configuration.name, configuration.predicate, {
    when: configuration.when,
  })
}

function project(value) {
  const configuration = configurationOf('project', value)

  return functional.project(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

function dimension(value) {
  const configuration = configurationOf('dimension', value)

  return functional.dimension(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

function measure(value) {
  const configuration = configurationOf('measure', value)

  return functional.measure(configuration.path, configuration.expression, {
    default: configuration.default,
  })
}

exports.GRAPH_DEFINITION_VERSION = functional.GRAPH_DEFINITION_VERSION
exports.fieldType = functional.fieldType
exports.nullable = functional.nullable
exports.hidden = functional.hidden
exports.source = functional.source
exports.field = functional.field
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
