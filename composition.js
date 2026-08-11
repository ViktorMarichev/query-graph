'use strict'

const BATCH_KEYS = new Set(['name', 'from', 'graph', 'to', 'parameter', 'cardinality', 'parameters', 'ordering'])
const COMPOSE_KEYS = new Set(['root', 'relations'])
const batchBrand = Symbol('BatchRelation')

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

function nonEmptyString(value, label) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${label} must be a non-empty string`)
  }
  return value
}

function relationalGraph(value, label) {
  if (value === null || typeof value !== 'object' || typeof value.compose !== 'function') {
    throw new TypeError(`${label} must be a RelationalQueryGraph`)
  }
  return value
}

function batchRelation(value) {
  const configuration = configurationOf('batchRelation', value, BATCH_KEYS)
  const name = nonEmptyString(configuration.name, 'batchRelation.name')
  if (name.includes('.')) {
    throw new TypeError('batchRelation.name must be a single projection path segment')
  }

  const from = nonEmptyString(configuration.from, 'batchRelation.from')
  const to = nonEmptyString(configuration.to, 'batchRelation.to')
  const graph = relationalGraph(configuration.graph, 'batchRelation.graph')
  const parameter = nonEmptyString(
    typeof configuration.parameter === 'string' ? configuration.parameter : configuration.parameter?.name,
    'batchRelation.parameter',
  )

  if (configuration.cardinality !== 'one' && configuration.cardinality !== 'many') {
    throw new TypeError("batchRelation.cardinality must be 'one' or 'many'")
  }
  if (
    configuration.parameters !== undefined &&
    (configuration.parameters === null ||
      typeof configuration.parameters !== 'object' ||
      Array.isArray(configuration.parameters))
  ) {
    throw new TypeError('batchRelation.parameters must be an object')
  }
  if (configuration.ordering !== undefined) {
    nonEmptyString(configuration.ordering, 'batchRelation.ordering')
  }

  return Object.freeze({
    name,
    from,
    graph,
    to,
    parameter,
    cardinality: configuration.cardinality,
    parameters: Object.freeze({ ...configuration.parameters }),
    ...(configuration.ordering === undefined ? {} : { ordering: configuration.ordering }),
    [batchBrand]: true,
  })
}

function composeGraph(value) {
  const configuration = configurationOf('composeGraph', value, COMPOSE_KEYS)
  const root = relationalGraph(configuration.root, 'composeGraph.root')
  if (!Array.isArray(configuration.relations)) {
    throw new TypeError('composeGraph.relations must be an array')
  }

  return configuration.relations.reduce((graph, relation, index) => {
    if (relation?.[batchBrand] !== true) {
      throw new TypeError(`composeGraph.relations[${index}] must be created by batchRelation`)
    }

    return graph.withBatchRelation(relation.graph, {
      name: relation.name,
      from: relation.from,
      to: relation.to,
      parameter: relation.parameter,
      cardinality: relation.cardinality,
      parameters: relation.parameters,
      ...(relation.ordering === undefined ? {} : { ordering: relation.ordering }),
    })
  }, root.compose())
}

module.exports = { batchRelation, composeGraph }
