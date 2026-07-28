'use strict'

const GRAPH_DEFINITION_VERSION = 3
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

function param(parameterReference) {
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

function relation(name, from, to, on, options = {}) {
  const definition = {
    name,
    from: referenceName(from, 'source'),
    to: referenceName(to, 'source'),
    on: asExpression(on),
  }
  if (options.required === true) {
    definition.required = true
  }
  if (options.cardinality === 'many') {
    definition.cardinality = 'many'
  }
  return definition
}

function constraint(name, predicate, options = {}) {
  const definition = {
    name,
    predicate: asExpression(predicate),
  }
  if (options.when !== undefined) {
    definition.when = {
      kind: 'parameterPresent',
      parameter: referenceName(options.when, 'parameter'),
    }
  }
  return definition
}

function project(path, expression, options = {}) {
  const definition = {
    path: typeof path === 'string' ? path.split('.') : [...path],
    expression: asExpression(expression),
  }
  if (options.default === true) {
    definition.selectedByDefault = true
  }
  return definition
}

function asc(expression, options = {}) {
  return orderBy('asc', expression, options)
}

function desc(expression, options = {}) {
  return orderBy('desc', expression, options)
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
    defaultOrderBy: content.defaultOrderBy,
  }
  Object.defineProperty(module, GRAPH_MODULE_BRAND, { value: true })

  return deepFreeze(module)
}

function defineGraph(configuration) {
  const content = composeDefinitionContent(configuration)

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
    defaultOrderBy: content.defaultOrderBy,
  })
}

function composeDefinitionContent(configuration) {
  const content = {
    sources: [],
    parameters: [],
    relations: [],
    constraints: [],
    projection: [],
    defaultOrderBy: [],
  }
  const indexes = {
    sources: new Map(),
    parameters: new Map(),
    relations: new Map(),
    constraints: new Map(),
    projection: new Map(),
    defaultOrderBy: new Set(),
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
  addNamedDefinitions(
    content.constraints,
    indexes.constraints,
    part.constraints,
    'constraint',
    owner,
    (constraint) => constraint.name,
  )
  addNamedDefinitions(content.projection, indexes.projection, part.projection, 'projection', owner, (projection) =>
    projection.path.join('.'),
  )

  for (const order of part.defaultOrderBy ?? []) {
    if (!indexes.defaultOrderBy.has(order)) {
      indexes.defaultOrderBy.add(order)
      content.defaultOrderBy.push(order)
    }
  }
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

function normalizeFieldSpec(specification) {
  if (typeof specification === 'string') {
    return fieldType(specification)
  }
  if (specification && typeof specification === 'object') {
    return fieldType(specification.scalarType, specification)
  }
  throw new TypeError('Field specification must contain a scalar type')
}

function parameterDefinition(name, scalarType, required) {
  assertScalarType(scalarType)
  const definition = { name, scalarType }
  if (required) {
    definition.required = true
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

exports.GRAPH_DEFINITION_VERSION = GRAPH_DEFINITION_VERSION
exports.fieldType = fieldType
exports.nullable = nullable
exports.hidden = hidden
exports.source = source
exports.field = field
exports.requiredParameter = requiredParameter
exports.optionalParameter = optionalParameter
exports.param = param
exports.literal = literal
exports.integer = integer
exports.decimal = decimal
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
exports.lower = lower
exports.upper = upper
exports.coalesce = coalesce
exports.concat = concat
exports.relation = relation
exports.constraint = constraint
exports.project = project
exports.asc = asc
exports.desc = desc
exports.defineGraphModule = defineGraphModule
exports.defineGraph = defineGraph
