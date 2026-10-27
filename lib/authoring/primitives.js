'use strict'

const { randomUUID } = require('node:crypto')
const { arrayOf, configurationOf, nonEmptyString, objectOf, optionalBoolean } = require('../configuration.js')
const { GROUP_VALUES_REF_BRAND, PARAMETER_REF_BRAND, SUMMARY_FIELD_BRAND } = require('./symbols.js')

const SCALAR_TYPES = new Set([
  'boolean',
  'int32',
  'int64',
  'float64',
  'decimal',
  'string',
  'date',
  'dateTime',
  'binary',
  'json',
])
const FIELD_TYPE_OPTIONS_KEYS = new Set(['nullable', 'selectable'])
const FIELD_SPECIFICATION_KEYS = new Set(['scalarType', 'nullable', 'selectable'])
const EXISTS_CONFIGURATION_KEYS = new Set(['from'])
const GROUPED_LIST_CONFIGURATION_KEYS = new Set(['key', 'value'])

function fieldType(scalarType, options = {}) {
  assertScalarType(scalarType)
  const configuration = configurationOf('fieldType', options, FIELD_TYPE_OPTIONS_KEYS)
  optionalBoolean(configuration.nullable, 'fieldType nullable')
  optionalBoolean(configuration.selectable, 'fieldType selectable')

  const definition = { scalarType }
  if (configuration.nullable === true) {
    definition.nullable = true
  }
  if (configuration.selectable === false) {
    definition.selectable = false
  }
  return definition
}

function nullable(specification) {
  return {
    ...normalizeFieldSpec(specification),
    nullable: true,
  }
}

function hidden(specification) {
  return {
    ...normalizeFieldSpec(specification),
    selectable: false,
  }
}

function source(key, fieldSpecifications) {
  nonEmptyString(key, 'source key')
  objectOf('source fields', fieldSpecifications)

  const fields = Object.entries(fieldSpecifications).map(([name, specification]) => ({
    name: nonEmptyString(name, 'source field name'),
    ...normalizeFieldSpec(specification),
  }))
  const sourceReference = { key, fields }

  Object.defineProperty(sourceReference, 'field', {
    enumerable: false,
    value(name) {
      return field(sourceReference, name)
    },
  })

  return sourceReference
}

function field(sourceReference, name) {
  const sourceKey = referenceName(sourceReference, 'source')
  nonEmptyString(name, 'field name')

  if (typeof sourceReference !== 'string' && !sourceReference.fields.some((candidate) => candidate.name === name)) {
    throw new TypeError(`Source ${sourceKey} does not define field ${name}`)
  }

  return {
    kind: 'field',
    source: sourceKey,
    field: name,
  }
}

function requiredParameter(name, scalarType) {
  return parameterDefinition(name, scalarType, true)
}

function optionalParameter(name, scalarType) {
  return parameterDefinition(name, scalarType, false)
}

function requiredListParameter(name, scalarType) {
  return parameterDefinition(name, scalarType, true, 'list')
}

function optionalListParameter(name, scalarType) {
  return parameterDefinition(name, scalarType, false, 'list')
}

function requiredGroupsParameter(name, types) {
  return groupedListParameterDefinition(name, types, true)
}

function optionalGroupsParameter(name, types) {
  return groupedListParameterDefinition(name, types, false)
}

function groupedListParameterDefinition(name, types, required) {
  const configuration = configurationOf('grouped list parameter', types, GROUPED_LIST_CONFIGURATION_KEYS)
  assertScalarType(configuration.key)
  const definition = parameterDefinition(name, configuration.value, required, 'groupedList')
  definition.keyType = configuration.key
  return definition
}

function param(parameterReference) {
  if (typeof parameterReference !== 'string' && parameterReference?.shape === 'list') {
    throw new TypeError(`List parameter ${parameterReference.name} cannot be used as a scalar expression`)
  }
  if (parameterReference?.shape === 'groupedList') {
    throw new TypeError(`Grouped list parameter ${parameterReference.name} cannot be used as a scalar expression`)
  }
  if (parameterReference?.[GROUP_VALUES_REF_BRAND] === true) {
    throw new TypeError('Group values cannot be used as a scalar expression')
  }

  return {
    kind: 'parameter',
    name: referenceName(parameterReference, 'parameter'),
  }
}

function literal(value) {
  if (value === null) {
    return literalExpression({ kind: 'null' })
  }
  if (typeof value === 'boolean') {
    return literalExpression({ kind: 'boolean', value })
  }
  if (typeof value === 'string') {
    return literalExpression({ kind: 'string', value })
  }
  if (typeof value === 'number') {
    return Number.isInteger(value) ? integer(value) : decimal(value)
  }

  throw new TypeError('Literal must be null, boolean, string, or number')
}

function integer(value) {
  if (!Number.isSafeInteger(value)) {
    throw new TypeError('Integer literal must be a safe JavaScript integer')
  }
  return literalExpression({ kind: 'integer', value })
}

function decimal(value) {
  const text = String(value)
  if (!/^-?(?:\d+(?:\.\d*)?|\.\d+)$/.test(text)) {
    throw new TypeError(`Invalid decimal literal ${text}`)
  }
  return literalExpression({ kind: 'decimal', value: text })
}

function eq(left, right) {
  return binaryExpression('eq', left, right)
}

function neq(left, right) {
  return binaryExpression('notEq', left, right)
}

function lt(left, right) {
  return binaryExpression('lessThan', left, right)
}

function lte(left, right) {
  return binaryExpression('lessThanOrEqual', left, right)
}

function gt(left, right) {
  return binaryExpression('greaterThan', left, right)
}

function gte(left, right) {
  return binaryExpression('greaterThanOrEqual', left, right)
}

function like(expression, pattern) {
  return {
    kind: 'like',
    expression: asExpression(expression),
    pattern: asExpression(pattern),
  }
}

function inList(expression, values) {
  return {
    kind: 'in',
    expression: asExpression(expression),
    values: arrayOf(values, 'inList values').map(asExpression),
  }
}

function inParameter(expression, parameterReference) {
  if (parameterReference?.[GROUP_VALUES_REF_BRAND] === true) {
    return {
      kind: 'inGroupValues',
      expression: asExpression(expression),
      scope: parameterReference.scope,
    }
  }
  if (typeof parameterReference !== 'string' && parameterReference?.shape !== 'list') {
    throw new TypeError(`Parameter ${parameterReference?.name ?? '<unknown>'} is not a list parameter`)
  }

  return {
    kind: 'inParameter',
    expression: asExpression(expression),
    parameter: referenceName(parameterReference, 'parameter'),
  }
}

function allGroups(parameterReference, predicate) {
  if (parameterReference?.[PARAMETER_REF_BRAND] !== true || parameterReference.shape !== 'groupedList') {
    throw new TypeError('allGroups requires a grouped list parameter')
  }
  const parameter = referenceName(parameterReference, 'parameter')
  assertScalarType(parameterReference.keyType)
  assertScalarType(parameterReference.scalarType)
  if (typeof predicate !== 'function') {
    throw new TypeError('allGroups predicate must be a function')
  }

  const scope = randomUUID()
  const key = Object.freeze({ kind: 'groupKey', scope })
  const values = { scope }
  Object.defineProperty(values, GROUP_VALUES_REF_BRAND, { value: true })
  Object.freeze(values)

  return {
    kind: 'allGroups',
    parameter,
    scope,
    predicate: asExpression(predicate({ key, values })),
  }
}

function and(...expressions) {
  return {
    kind: 'and',
    expressions: expressions.map(asExpression),
  }
}

function or(...expressions) {
  return {
    kind: 'or',
    expressions: expressions.map(asExpression),
  }
}

function not(expression) {
  return unaryExpression('not', expression)
}

function isNull(expression) {
  return unaryExpression('isNull', expression)
}

function isNotNull(expression) {
  return unaryExpression('isNotNull', expression)
}

function exists(sourceReference, predicate, options) {
  const expression = {
    kind: 'exists',
    source: referenceName(sourceReference, 'source'),
  }

  if (options !== undefined) {
    const configuration = configurationOf('exists', options, EXISTS_CONFIGURATION_KEYS)
    if (configuration.from === undefined) {
      throw new TypeError('exists requires configuration field "from"')
    }
    expression.from = referenceName(configuration.from, 'source')
  }

  if (predicate !== undefined) {
    expression.predicate = asExpression(predicate)
  }

  return expression
}

function call(name, ...arguments_) {
  return {
    kind: 'function',
    name,
    arguments: arguments_.map(asExpression),
  }
}

function lower(expression) {
  return call('lower', expression)
}

function upper(expression) {
  return call('upper', expression)
}

function coalesce(first, second, ...rest) {
  return call('coalesce', first, second, ...rest)
}

function concat(first, ...rest) {
  return call('concat', first, ...rest)
}

function divide(left, right) {
  return call('divide', left, right)
}

function truncate(expression) {
  return call('truncate', expression)
}

function modulo(left, right) {
  return call('modulo', left, right)
}

function aggregate(functionName, expression) {
  const definition = {
    kind: 'aggregate',
    function: functionName,
  }
  if (expression !== undefined) {
    definition.expression = asExpression(expression)
  }
  return definition
}

function count(expression) {
  return aggregate('count', expression)
}

function countDistinct(expression) {
  return aggregate('countDistinct', expression)
}

function sum(expression) {
  return aggregate('sum', expression)
}

function average(expression) {
  return aggregate('average', expression)
}

function minimum(expression) {
  return aggregate('minimum', expression)
}

function maximum(expression) {
  return aggregate('maximum', expression)
}

function normalizeFieldSpec(specification) {
  if (typeof specification === 'string') {
    return fieldType(specification)
  }
  if (specification && typeof specification === 'object' && !Array.isArray(specification)) {
    const configuration = configurationOf('field specification', specification, FIELD_SPECIFICATION_KEYS)
    return fieldType(configuration.scalarType, {
      nullable: configuration.nullable,
      selectable: configuration.selectable,
    })
  }
  throw new TypeError('Field specification must contain a scalar type')
}

function parameterDefinition(name, scalarType, required, shape = 'scalar') {
  nonEmptyString(name, 'parameter name')
  assertScalarType(scalarType)
  const definition = { name, scalarType }
  if (required) {
    definition.required = true
  }
  if (shape !== 'scalar') {
    definition.shape = shape
  }
  Object.defineProperty(definition, PARAMETER_REF_BRAND, { value: true })
  return definition
}

function literalExpression(value) {
  return {
    kind: 'literal',
    value,
  }
}

function binaryExpression(kind, left, right) {
  return {
    kind,
    left: asExpression(left),
    right: asExpression(right),
  }
}

function unaryExpression(kind, expression) {
  return {
    kind,
    expression: asExpression(expression),
  }
}

function asExpression(value) {
  if (value?.[GROUP_VALUES_REF_BRAND] === true) {
    throw new TypeError('Group values cannot be used as a scalar expression')
  }
  if (value?.[SUMMARY_FIELD_BRAND] === true) {
    return value.expression
  }
  if (value?.[PARAMETER_REF_BRAND] === true) {
    return param(value)
  }
  if (value && typeof value === 'object' && typeof value.kind === 'string') {
    return value
  }
  return literal(value)
}

function referenceName(reference, type) {
  if (typeof reference === 'string') {
    return nonEmptyString(reference, `${type} name`)
  }
  if (type === 'source') {
    if (reference && typeof reference.key === 'string') {
      return nonEmptyString(reference.key, 'source key')
    }
    throw new TypeError('Invalid source reference')
  }
  if (reference && typeof reference.name === 'string') {
    return nonEmptyString(reference.name, `${type} name`)
  }
  throw new TypeError(`Invalid ${type} reference`)
}

function copySource(sourceReference) {
  return {
    key: sourceReference.key,
    fields: sourceReference.fields.map((fieldDefinition) => ({ ...fieldDefinition })),
  }
}

function assertScalarType(scalarType) {
  if (!SCALAR_TYPES.has(scalarType)) {
    throw new TypeError(`Unknown scalar type ${scalarType}`)
  }
}

module.exports = {
  allGroups,
  and,
  asExpression,
  average,
  coalesce,
  concat,
  copySource,
  count,
  countDistinct,
  decimal,
  divide,
  eq,
  exists,
  fieldType,
  gt,
  gte,
  hidden,
  inList,
  inParam: inParameter,
  inParameter,
  integer,
  isNotNull,
  isNull,
  like,
  literal,
  lower,
  lt,
  lte,
  maximum,
  minimum,
  modulo,
  neq,
  not,
  nullable,
  optionalGroupsParam: optionalGroupsParameter,
  optionalGroupsParameter,
  optionalListParam: optionalListParameter,
  optionalListParameter,
  optionalParam: optionalParameter,
  optionalParameter,
  or,
  param,
  referenceName,
  requiredGroupsParam: requiredGroupsParameter,
  requiredGroupsParameter,
  requiredListParam: requiredListParameter,
  requiredListParameter,
  requiredParam: requiredParameter,
  requiredParameter,
  source,
  sum,
  truncate,
  upper,
}
