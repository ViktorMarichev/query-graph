'use strict'

const { randomUUID } = require('node:crypto')
const { configurationOf, nonEmptyString, optionalArray, optionalEnum } = require('../configuration.js')
const { asExpression, copySource } = require('./primitives.js')
const { constraint, defineGraph } = require('./definition.js')

const TRAVERSAL_KEYS = ['source', 'modules', 'key', 'where', 'start', 'step', 'cycles']
const PATH_REF = Symbol('traversalPath')

function buildTraversal(configuration, factory) {
  const { source, key, step } = configuration
  if (!source || typeof source.field !== 'function') {
    throw new TypeError(`${factory} source must be a source reference`)
  }
  if (source.traversal) throw new TypeError(`${factory} cannot nest traversal sources`)
  if (key?.kind !== 'field' || key.source !== source.key) {
    throw new TypeError(`${factory} key must be a field of its source`)
  }
  if (typeof step !== 'function') throw new TypeError(`${factory} step must be a function`)
  if (configuration.start === undefined) throw new TypeError(`${factory} requires a start predicate`)
  optionalArray(configuration.modules, `${factory}.modules`)
  optionalEnum(configuration.cycles, new Set(['skip']), `${factory} cycles`)

  const scope = randomUUID()
  const reference = (side) =>
    Object.freeze({
      field(name) {
        source.field(name)
        return { kind: 'traversalField', scope, side, field: name }
      },
    })
  const input = defineGraph({
    name: `${factory}Input`,
    root: source,
    sources: [source],
    modules: configuration.modules,
    constraints: configuration.where === undefined ? [] : [constraint({ predicate: configuration.where })],
  })
  if (input.projection.fields.length || input.projection.objects.length || input.orderings.length) {
    throw new TypeError(`${factory} modules may not define projection, objects or orderings`)
  }
  return {
    scope,
    input,
    key: key.field,
    start: asExpression(configuration.start),
    step: asExpression(step({ current: reference('current'), next: reference('next') })),
    cycles: configuration.cycles ?? 'skip',
  }
}

function traversalSource(value) {
  const configuration = configurationOf('traversalSource', value, new Set([...TRAVERSAL_KEYS, 'name', 'path']))
  const name = nonEmptyString(configuration.name, 'traversalSource name')
  const traversal = buildTraversal(configuration, 'traversalSource')
  if (configuration.path !== undefined) {
    const path = configurationOf('traversalSource.path', configuration.path, new Set(['prefix', 'node']))
    const fieldName = (expression, label) => {
      if (expression?.kind !== 'field' || expression.source !== configuration.source.key) {
        throw new TypeError(`traversalSource.path.${label} must be a field of its input source`)
      }
      configuration.source.field(expression.field)
      return expression.field
    }
    traversal.path = {
      node: fieldName(path.node, 'node'),
      ...(path.prefix === undefined ? {} : { prefix: fieldName(path.prefix, 'prefix') }),
    }
  }
  const result = { ...copySource(configuration.source), key: name, traversal }
  Object.defineProperties(result, {
    field: {
      value(field) {
        configuration.source.field(field)
        return { kind: 'field', source: name, field }
      },
    },
    rootField: {
      value(field) {
        configuration.source.field(field)
        return { kind: 'traversalRoot', source: name, field }
      },
    },
    level: { value: () => ({ kind: 'traversalLevel', source: name }) },
    path: {
      value() {
        if (!traversal.path) throw new TypeError('traversalSource has no path configuration')
        return Object.freeze({ [PATH_REF]: true, source: name })
      },
    },
  })
  return result
}

function pathEquals(path, parameter) {
  if (path?.[PATH_REF] !== true) throw new TypeError('pathEquals requires a traversal path reference')
  if (parameter?.shape !== 'list' || typeof parameter.name !== 'string') {
    throw new TypeError('pathEquals requires a list parameter')
  }
  return { kind: 'traversalPathEquals', source: path.source, parameter: parameter.name }
}

module.exports = { TRAVERSAL_KEYS, buildTraversal, traversalSource, pathEquals }
