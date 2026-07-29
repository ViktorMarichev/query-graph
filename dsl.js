'use strict'

const GRAPH_DEFINITION_VERSION = 9
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
const GRAPH_MODULE_BRAND = Symbol('GraphModule')
const SUMMARY_FIELD_BRAND = Symbol('SummaryField')

const RELATION_CONFIGURATION_KEYS = new Set(['name', 'from', 'to', 'on', 'required', 'cardinality', 'selection'])
const CONSTRAINT_CONFIGURATION_KEYS = new Set(['predicate', 'when'])
const PROJECTION_CONFIGURATION_KEYS = new Set(['path', 'expression', 'default'])
const EXISTS_CONFIGURATION_KEYS = new Set(['from'])
const ORDERING_CONFIGURATION_KEYS = new Set(['name', 'by', 'default'])

function configurationOf(factory, value, allowedKeys) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${factory} expects a configuration object`)
  }

  const unknownKeys = Reflect.ownKeys(value).filter((key) => typeof key !== 'string' || !allowedKeys.has(key))
  if (unknownKeys.length > 0) {
    const label = unknownKeys.length === 1 ? 'field' : 'fields'
    const keys = unknownKeys.map((key) => JSON.stringify(String(key))).join(', ')
    throw new TypeError(`${factory} received unknown configuration ${label} ${keys}`)
  }

  return value
}

function fieldType(scalarType, options = {}) {
  assertScalarType(scalarType)

  const definition = { scalarType }
  if (options.nullable === true) {
    definition.nullable = true
  }
  if (options.selectable === false) {
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
  const fields = Object.entries(fieldSpecifications).map(([name, specification]) => ({
    name,
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

function param(parameterReference) {
  if (typeof parameterReference !== 'string' && parameterReference?.shape === 'list') {
    throw new TypeError(`List parameter ${parameterReference.name} cannot be used as a scalar expression`)
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
    values: values.map(asExpression),
  }
}

function inParameter(expression, parameterReference) {
  if (typeof parameterReference !== 'string' && parameterReference?.shape !== 'list') {
    throw new TypeError(`Parameter ${parameterReference?.name ?? '<unknown>'} is not a list parameter`)
  }

  return {
    kind: 'inParameter',
    expression: asExpression(expression),
    parameter: referenceName(parameterReference, 'parameter'),
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

function relation(value) {
  const configuration = configurationOf('relation', value, RELATION_CONFIGURATION_KEYS)
  const definition = {
    name: configuration.name,
    from: referenceName(configuration.from, 'source'),
    to: referenceName(configuration.to, 'source'),
    on: asExpression(configuration.on),
  }
  if (configuration.required === true) {
    definition.required = true
  }
  if (configuration.cardinality === 'many') {
    definition.cardinality = 'many'
  }
  if (configuration.selection !== undefined) {
    definition.selection = configuration.selection
  }
  return definition
}

function constraint(value) {
  const configuration = configurationOf('constraint', value, CONSTRAINT_CONFIGURATION_KEYS)
  const definition = {
    predicate: asExpression(configuration.predicate),
  }
  if (configuration.when !== undefined) {
    definition.when = {
      kind: 'parameterPresent',
      parameter: referenceName(configuration.when, 'parameter'),
    }
  }
  return definition
}

function project(value) {
  const configuration = configurationOf('project', value, PROJECTION_CONFIGURATION_KEYS)
  const definition = {
    path: typeof configuration.path === 'string' ? configuration.path.split('.') : [...configuration.path],
    expression: asExpression(configuration.expression),
  }
  if (configuration.default === true) {
    definition.selectedByDefault = true
  }
  return definition
}

function dimension(value) {
  return summaryField('dimension', configurationOf('dimension', value, PROJECTION_CONFIGURATION_KEYS))
}

function measure(value) {
  return summaryField('measure', configurationOf('measure', value, PROJECTION_CONFIGURATION_KEYS))
}

function summaryField(role, configuration) {
  const definition = {
    ...project(configuration),
    role,
  }
  Object.defineProperty(definition, SUMMARY_FIELD_BRAND, {
    value: true,
  })

  return definition
}

function asc(expression, options = {}) {
  return orderBy('asc', expression, options)
}

function desc(expression, options = {}) {
  return orderBy('desc', expression, options)
}

function ordering(value) {
  const configuration = configurationOf('ordering', value, ORDERING_CONFIGURATION_KEYS)
  if (typeof configuration.name !== 'string' || configuration.name.trim() === '') {
    throw new TypeError('ordering name must be a non-empty string')
  }
  if (!Array.isArray(configuration.by) || configuration.by.length === 0) {
    throw new TypeError('ordering by must contain at least one order expression')
  }
  if (configuration.default !== undefined && typeof configuration.default !== 'boolean') {
    throw new TypeError('ordering default must be a boolean')
  }

  const definition = {
    name: configuration.name,
    orderBy: [...configuration.by],
  }
  if (configuration.default === true) {
    definition.default = true
  }

  return definition
}

function firstBy(firstOrder, ...rest) {
  if (firstOrder === undefined) {
    throw new TypeError('firstBy requires at least one order expression')
  }

  return {
    kind: 'firstBy',
    orderBy: [firstOrder, ...rest],
  }
}

function defineGraphModule(configuration) {
  const content = composeDefinitionContent(configuration)

  const module = {
    name: configuration.name,
    sources: content.sources,
    parameters: content.parameters,
    relations: content.relations,
    constraints: content.constraints,
    projection: content.projection,
    orderings: content.orderings,
  }
  Object.defineProperty(module, GRAPH_MODULE_BRAND, { value: true })

  return deepFreeze(module)
}

function defineGraph(configuration) {
  return buildGraphDefinition(configuration, 'record')
}

function defineSummaryGraph(configuration) {
  if (configuration.projection !== undefined) {
    throw new TypeError('Summary graph uses dimensions and measures instead of projection')
  }

  return buildGraphDefinition(
    {
      ...configuration,
      projection: [...(configuration.dimensions ?? []), ...(configuration.measures ?? [])],
    },
    'summary',
  )
}

function buildGraphDefinition(configuration, mode) {
  const content = composeDefinitionContent(configuration)
  validateProjectionMode(content.projection, mode)

  return deepFreeze({
    schemaVersion: GRAPH_DEFINITION_VERSION,
    name: configuration.name,
    root: referenceName(configuration.root, 'source'),
    sources: content.sources.map(copySource),
    parameters: content.parameters,
    relations: content.relations,
    constraints: content.constraints,
    projection: {
      fields: content.projection,
    },
    orderings: content.orderings,
  })
}

function validateProjectionMode(projection, mode) {
  if (mode === 'summary') {
    if (projection.length === 0) {
      throw new TypeError('Summary graph must define at least one dimension or measure')
    }
    const invalid = projection.find((field) => field.role !== 'dimension' && field.role !== 'measure')
    if (invalid !== undefined) {
      throw new TypeError(
        `Summary graph projection field ${JSON.stringify(invalid.path.join('.'))} has no summary role`,
      )
    }
    return
  }

  const summaryField = projection.find((field) => field.role === 'dimension' || field.role === 'measure')
  if (summaryField !== undefined) {
    throw new TypeError(
      `Record graph projection field ${JSON.stringify(summaryField.path.join('.'))} has a summary role`,
    )
  }
}

function composeDefinitionContent(configuration) {
  const content = {
    sources: [],
    parameters: [],
    relations: [],
    constraints: [],
    projection: [],
    orderings: [],
  }
  const indexes = {
    sources: new Map(),
    parameters: new Map(),
    relations: new Map(),
    constraints: new Set(),
    projection: new Map(),
    orderings: new Map(),
  }

  for (const [index, module] of (configuration.modules ?? []).entries()) {
    if (module?.[GRAPH_MODULE_BRAND] !== true) {
      throw new TypeError(`Graph module at index ${index} was not created by defineGraphModule`)
    }

    addDefinitionPart(content, indexes, module, `graph module ${JSON.stringify(module.name)}`)
  }

  addDefinitionPart(content, indexes, configuration, `definition ${JSON.stringify(configuration.name)}`)
  return content
}

function addDefinitionPart(content, indexes, part, owner) {
  addNamedDefinitions(content.sources, indexes.sources, part.sources, 'source', owner, (source) => source.key)
  addNamedDefinitions(
    content.parameters,
    indexes.parameters,
    part.parameters,
    'parameter',
    owner,
    (parameter) => parameter.name,
  )
  addNamedDefinitions(
    content.relations,
    indexes.relations,
    part.relations,
    'relation',
    owner,
    (relation) => relation.name,
  )
  addIdentityDefinitions(content.constraints, indexes.constraints, part.constraints)
  addNamedDefinitions(content.projection, indexes.projection, part.projection, 'projection', owner, (projection) =>
    projection.path.join('.'),
  )

  addNamedDefinitions(
    content.orderings,
    indexes.orderings,
    part.orderings,
    'ordering',
    owner,
    (orderingDefinition) => orderingDefinition.name,
  )
}

function addNamedDefinitions(target, index, definitions, type, owner, getKey) {
  for (const definition of definitions ?? []) {
    const key = getKey(definition)
    const existing = index.get(key)

    if (existing === undefined) {
      index.set(key, { definition, owner })
      target.push(definition)
      continue
    }

    if (existing.definition !== definition) {
      throw new TypeError(`Conflicting ${type} ${JSON.stringify(key)} from ${existing.owner} and ${owner}`)
    }
  }
}

function addIdentityDefinitions(target, index, definitions) {
  for (const definition of definitions ?? []) {
    if (!index.has(definition)) {
      index.add(definition)
      target.push(definition)
    }
  }
}

function normalizeFieldSpec(specification) {
  if (typeof specification === 'string') {
    return fieldType(specification)
  }
  if (specification && typeof specification === 'object') {
    return fieldType(specification.scalarType, specification)
  }
  throw new TypeError('Field specification must contain a scalar type')
}

function parameterDefinition(name, scalarType, required, shape = 'scalar') {
  assertScalarType(scalarType)
  const definition = { name, scalarType }
  if (required) {
    definition.required = true
  }
  if (shape === 'list') {
    definition.shape = 'list'
  }
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
  if (value?.[SUMMARY_FIELD_BRAND] === true) {
    return value.expression
  }
  if (value && typeof value === 'object' && typeof value.kind === 'string') {
    return value
  }
  return literal(value)
}

function orderBy(direction, expression, options) {
  const definition = {
    expression: asExpression(expression),
    direction,
  }
  if (options.nulls !== undefined) {
    definition.nulls = options.nulls
  }
  return definition
}

function referenceName(reference, type) {
  if (typeof reference === 'string') {
    return reference
  }
  if (type === 'source') {
    if (reference && typeof reference.key === 'string') {
      return reference.key
    }
    throw new TypeError('Invalid source reference')
  }
  if (reference && typeof reference.name === 'string') {
    return reference.name
  }
  throw new TypeError(`Invalid ${type} reference`)
}

function copySource(sourceReference) {
  return {
    key: sourceReference.key,
    fields: sourceReference.fields.map((fieldDefinition) => ({ ...fieldDefinition })),
  }
}

function deepFreeze(value, seen = new WeakSet()) {
  if (value === null || typeof value !== 'object' || seen.has(value)) {
    return value
  }

  seen.add(value)
  for (const property of Reflect.ownKeys(value)) {
    deepFreeze(value[property], seen)
  }

  return Object.freeze(value)
}

function assertScalarType(scalarType) {
  if (!SCALAR_TYPES.has(scalarType)) {
    throw new TypeError(`Unknown scalar type ${scalarType}`)
  }
}

exports.fieldType = fieldType
exports.nullable = nullable
exports.hidden = hidden
exports.source = source
exports.requiredParameter = requiredParameter
exports.optionalParameter = optionalParameter
exports.param = param
exports.literal = literal
exports.integer = integer
exports.decimal = decimal
exports.requiredListParameter = requiredListParameter
exports.optionalListParameter = optionalListParameter
exports.eq = eq
exports.neq = neq
exports.lt = lt
exports.lte = lte
exports.gt = gt
exports.gte = gte
exports.like = like
exports.inList = inList
exports.and = and
exports.or = or
exports.not = not
exports.isNull = isNull
exports.isNotNull = isNotNull
exports.inParameter = inParameter
exports.exists = exists
exports.lower = lower
exports.upper = upper
exports.coalesce = coalesce
exports.concat = concat
exports.count = count
exports.countDistinct = countDistinct
exports.sum = sum
exports.average = average
exports.minimum = minimum
exports.maximum = maximum
exports.relation = relation
exports.constraint = constraint
exports.ordering = ordering
exports.project = project
exports.dimension = dimension
exports.measure = measure
exports.asc = asc
exports.desc = desc
exports.defineGraphModule = defineGraphModule
exports.defineGraph = defineGraph
exports.defineSummaryGraph = defineSummaryGraph
exports.firstBy = firstBy
