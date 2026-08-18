'use strict'

const {
  configurationOf,
  nonEmptyString,
  objectOf,
  optionalArray,
  optionalBoolean,
  optionalEnum,
} = require('../configuration.js')
const { asExpression, copySource, referenceName } = require('./primitives.js')
const { GRAPH_MODULE_BRAND, SUMMARY_FIELD_BRAND } = require('./symbols.js')

const GRAPH_DEFINITION_VERSION = 10
const RELATION_CONFIGURATION_KEYS = new Set(['name', 'from', 'to', 'on', 'required', 'cardinality', 'selection'])
const CONSTRAINT_CONFIGURATION_KEYS = new Set(['predicate', 'when'])
const PROJECTION_CONFIGURATION_KEYS = new Set(['path', 'expression', 'default'])
const PROJECTION_OBJECT_CONFIGURATION_KEYS = new Set(['path', 'presence'])
const ORDERING_CONFIGURATION_KEYS = new Set(['name', 'by', 'default'])
const ORDER_BY_OPTIONS_KEYS = new Set(['nulls'])
const GRAPH_MODULE_CONFIGURATION_KEYS = new Set([
  'name',
  'modules',
  'sources',
  'parameters',
  'relations',
  'constraints',
  'projection',
  'objects',
  'orderings',
])
const GRAPH_CONFIGURATION_KEYS = new Set([...GRAPH_MODULE_CONFIGURATION_KEYS, 'root'])
const SUMMARY_GRAPH_CONFIGURATION_KEYS = new Set([
  'name',
  'root',
  'modules',
  'sources',
  'parameters',
  'relations',
  'constraints',
  'dimensions',
  'measures',
  'orderings',
])
const RELATION_CARDINALITIES = new Set(['one', 'many'])
const NULLS_ORDERS = new Set(['first', 'last'])
const DEFINITION_LIST_KEYS = [
  'modules',
  'sources',
  'parameters',
  'relations',
  'constraints',
  'projection',
  'objects',
  'orderings',
]

function relation(value) {
  const configuration = configurationOf('relation', value, RELATION_CONFIGURATION_KEYS)
  nonEmptyString(configuration.name, 'relation name')
  optionalBoolean(configuration.required, 'relation required')
  optionalEnum(configuration.cardinality, RELATION_CARDINALITIES, 'relation cardinality')

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
  optionalBoolean(configuration.default, 'project default')
  const definition = {
    path: projectionPath(configuration.path, 'project path'),
    expression: asExpression(configuration.expression),
  }
  if (configuration.default === true) {
    definition.selectedByDefault = true
  }
  return definition
}

function projectObject(value) {
  const configuration = configurationOf('projectObject', value, PROJECTION_OBJECT_CONFIGURATION_KEYS)
  return {
    path: projectionPath(configuration.path, 'projectObject path'),
    presence: asExpression(configuration.presence),
  }
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
  nonEmptyString(configuration.name, 'ordering name')
  if (!Array.isArray(configuration.by) || configuration.by.length === 0) {
    throw new TypeError('ordering by must contain at least one order expression')
  }
  optionalBoolean(configuration.default, 'ordering default')

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

function defineGraphModule(value) {
  const configuration = definitionConfiguration('defineGraphModule', value, GRAPH_MODULE_CONFIGURATION_KEYS)
  const content = composeDefinitionContent(configuration)

  const module = {
    name: configuration.name,
    sources: content.sources,
    parameters: content.parameters,
    relations: content.relations,
    constraints: content.constraints,
    projection: content.projection,
    objects: content.objects,
    orderings: content.orderings,
  }
  Object.defineProperty(module, GRAPH_MODULE_BRAND, { value: true })

  return deepFreeze(module)
}

function defineGraph(value) {
  const configuration = definitionConfiguration('defineGraph', value, GRAPH_CONFIGURATION_KEYS, true)
  return buildGraphDefinition(configuration, 'record')
}

function defineSummaryGraph(value) {
  objectOf('defineSummaryGraph configuration', value)
  if (Object.prototype.hasOwnProperty.call(value, 'projection')) {
    throw new TypeError('Summary graph uses dimensions and measures instead of projection')
  }

  const configuration = configurationOf('defineSummaryGraph', value, SUMMARY_GRAPH_CONFIGURATION_KEYS)
  validateCommonDefinitionConfiguration('defineSummaryGraph', configuration, true)
  optionalArray(configuration.dimensions, 'defineSummaryGraph.dimensions')
  optionalArray(configuration.measures, 'defineSummaryGraph.measures')

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
  validateProjectionMode(content.projection, content.objects, mode)

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
      objects: content.objects,
    },
    orderings: content.orderings,
  })
}

function definitionConfiguration(factory, value, allowedKeys, requiresRoot = false) {
  const configuration = configurationOf(factory, value, allowedKeys)
  validateCommonDefinitionConfiguration(factory, configuration, requiresRoot)
  return configuration
}

function validateCommonDefinitionConfiguration(factory, configuration, requiresRoot) {
  nonEmptyString(configuration.name, `${factory} name`)
  if (requiresRoot) {
    referenceName(configuration.root, 'source')
  }
  for (const key of DEFINITION_LIST_KEYS) {
    optionalArray(configuration[key], `${factory}.${key}`)
  }
}

function validateProjectionMode(projection, objects, mode) {
  if (mode === 'summary') {
    if (objects.length > 0) {
      throw new TypeError('Summary graph cannot define projection objects')
    }
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

  const summaryFieldDefinition = projection.find((field) => field.role === 'dimension' || field.role === 'measure')
  if (summaryFieldDefinition !== undefined) {
    throw new TypeError(
      `Record graph projection field ${JSON.stringify(summaryFieldDefinition.path.join('.'))} has a summary role`,
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
    objects: [],
    orderings: [],
  }
  const indexes = {
    sources: new Map(),
    parameters: new Map(),
    relations: new Map(),
    constraints: new Set(),
    projection: new Map(),
    objects: new Map(),
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
    (relationDefinition) => relationDefinition.name,
  )
  addIdentityDefinitions(content.constraints, indexes.constraints, part.constraints)
  addNamedDefinitions(content.projection, indexes.projection, part.projection, 'projection', owner, (projection) =>
    projection.path.join('.'),
  )
  addNamedDefinitions(content.objects, indexes.objects, part.objects, 'projection object', owner, (object) =>
    object.path.join('.'),
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

function projectionPath(path, label) {
  if (typeof path === 'string') {
    return path.split('.')
  }
  if (!Array.isArray(path) || path.some((segment) => typeof segment !== 'string')) {
    throw new TypeError(`${label} must be a string or an array of strings`)
  }
  return [...path]
}

function orderBy(direction, expression, options) {
  const configuration = configurationOf(`${direction} order`, options, ORDER_BY_OPTIONS_KEYS)
  optionalEnum(configuration.nulls, NULLS_ORDERS, `${direction} nulls`)

  const definition = {
    expression: asExpression(expression),
    direction,
  }
  if (configuration.nulls !== undefined) {
    definition.nulls = configuration.nulls
  }
  return definition
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

module.exports = {
  GRAPH_DEFINITION_VERSION,
  asc,
  constraint,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  desc,
  dimension,
  firstBy,
  measure,
  ordering,
  project,
  projectObject,
  relation,
}
