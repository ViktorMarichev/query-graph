'use strict'

const objectDsl = require('./dsl.js')

const GRAPH_DEFINITION_VERSION = 9

function field(sourceReference, name) {
  if (typeof sourceReference === 'string') {
    return {
      kind: 'field',
      source: sourceReference,
      field: name,
    }
  }

  return sourceReference.field(name)
}

function relation(name, from, to, on, options = {}) {
  return objectDsl.relation({
    name,
    from,
    to,
    on,
    required: options.required,
    cardinality: options.cardinality,
    selection: options.selection,
  })
}

function constraint(predicate, options = {}) {
  return objectDsl.constraint({
    predicate,
    when: options.when,
  })
}

function project(path, expression, options = {}) {
  return objectDsl.project({
    path,
    expression,
    default: options.default,
  })
}

function dimension(path, expression, options = {}) {
  return objectDsl.dimension({
    path,
    expression,
    default: options.default,
  })
}

function measure(path, expression, options = {}) {
  return objectDsl.measure({
    path,
    expression,
    default: options.default,
  })
}

exports.GRAPH_DEFINITION_VERSION = GRAPH_DEFINITION_VERSION
exports.fieldType = objectDsl.fieldType
exports.nullable = objectDsl.nullable
exports.hidden = objectDsl.hidden
exports.source = objectDsl.source
exports.field = field
exports.requiredParameter = objectDsl.requiredParameter
exports.optionalParameter = objectDsl.optionalParameter
exports.param = objectDsl.param
exports.literal = objectDsl.literal
exports.integer = objectDsl.integer
exports.decimal = objectDsl.decimal
exports.requiredListParameter = objectDsl.requiredListParameter
exports.optionalListParameter = objectDsl.optionalListParameter
exports.eq = objectDsl.eq
exports.neq = objectDsl.neq
exports.lt = objectDsl.lt
exports.lte = objectDsl.lte
exports.gt = objectDsl.gt
exports.gte = objectDsl.gte
exports.like = objectDsl.like
exports.inList = objectDsl.inList
exports.and = objectDsl.and
exports.or = objectDsl.or
exports.not = objectDsl.not
exports.isNull = objectDsl.isNull
exports.isNotNull = objectDsl.isNotNull
exports.inParameter = objectDsl.inParameter
exports.exists = objectDsl.exists
exports.lower = objectDsl.lower
exports.upper = objectDsl.upper
exports.coalesce = objectDsl.coalesce
exports.concat = objectDsl.concat
exports.count = objectDsl.count
exports.countDistinct = objectDsl.countDistinct
exports.sum = objectDsl.sum
exports.average = objectDsl.average
exports.minimum = objectDsl.minimum
exports.maximum = objectDsl.maximum
exports.relation = relation
exports.constraint = constraint
exports.ordering = objectDsl.ordering
exports.project = project
exports.dimension = dimension
exports.measure = measure
exports.asc = objectDsl.asc
exports.desc = objectDsl.desc
exports.defineGraphModule = objectDsl.defineGraphModule
exports.defineGraph = objectDsl.defineGraph
exports.defineSummaryGraph = objectDsl.defineSummaryGraph
exports.firstBy = objectDsl.firstBy
