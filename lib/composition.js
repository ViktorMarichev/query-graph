'use strict'

const { arrayOf, configurationOf, nonEmptyString, objectOf, optionalEnum } = require('./configuration.js')

const BATCH_QUERY_KEYS = new Set(['graph', 'key'])
const BATCH_QUERY_KEY_KEYS = new Set(['path', 'parameter'])
const BATCH_RELATION_KEYS = new Set(['name', 'from', 'query', 'cardinality', 'parameters', 'ordering'])
const COMPOSE_KEYS = new Set(['root', 'relations'])
const BATCH_CARDINALITIES = new Set(['one', 'many'])
const batchQueryBrand = Symbol('BatchQuery')
const batchBrand = Symbol('BatchRelation')

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
  optionalEnum(configuration.cardinality, BATCH_CARDINALITIES, 'batchRelation.cardinality')
  if (configuration.cardinality === undefined) {
    throw new TypeError("batchRelation.cardinality must be 'one' or 'many'")
  }
  if (configuration.parameters !== undefined) {
    objectOf('batchRelation.parameters', configuration.parameters)
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
  const relations = arrayOf(configuration.relations, 'composeGraph.relations')

  return relations.reduce((graph, relation, index) => {
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
