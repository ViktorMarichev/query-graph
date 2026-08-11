'use strict'

const BATCH_QUERY_KEYS = new Set(['graph', 'key'])
const BATCH_QUERY_KEY_KEYS = new Set(['path', 'parameter'])
const BATCH_RELATION_KEYS = new Set(['name', 'from', 'query', 'cardinality', 'parameters', 'ordering'])
const COMPOSE_KEYS = new Set(['root', 'relations'])
const batchQueryBrand = Symbol('BatchQuery')
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

function batchQueryOf(value, label) {
  if (value === null || typeof value !== 'object' || value[batchQueryBrand] !== true) {
    throw new TypeError(`${label} must be created by batchQuery`)
  }
  return value
}

function batchQuery(value) {
  const configuration = configurationOf('batchQuery', value, BATCH_QUERY_KEYS)
  const graph = relationalGraph(configuration.graph, 'batchQuery.graph')
  const keyConfiguration = configurationOf('batchQuery.key', configuration.key, BATCH_QUERY_KEY_KEYS)
  const path = nonEmptyString(keyConfiguration.path, 'batchQuery.key.path')
  const parameter = nonEmptyString(
    typeof keyConfiguration.parameter === 'string' ? keyConfiguration.parameter : keyConfiguration.parameter?.name,
    'batchQuery.key.parameter',
  )

  return Object.freeze({
    graph,
    key: Object.freeze({
      path,
      parameter,
    }),
    [batchQueryBrand]: true,
  })
}

function batchRelation(value) {
  const configuration = configurationOf('batchRelation', value, BATCH_RELATION_KEYS)
  const name = nonEmptyString(configuration.name, 'batchRelation.name')
  if (name.includes('.')) {
    throw new TypeError('batchRelation.name must be a single projection path segment')
  }

  const from = nonEmptyString(configuration.from, 'batchRelation.from')
  const query = batchQueryOf(configuration.query, 'batchRelation.query')

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
    query,
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

    return graph.withBatchRelation(relation.query.graph, {
      name: relation.name,
      from: relation.from,
      to: relation.query.key.path,
      parameter: relation.query.key.parameter,
      cardinality: relation.cardinality,
      parameters: relation.parameters,
      ...(relation.ordering === undefined ? {} : { ordering: relation.ordering }),
    })
  }, root.compose())
}

module.exports = { batchQuery, batchRelation, composeGraph }
