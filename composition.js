'use strict'

const BATCH_KEYS = new Set(['name', 'from', 'graph', 'to', 'parameter', 'cardinality', 'parameters', 'ordering'])
const COMPOSE_KEYS = new Set(['root', 'relations'])
const batchBrand = Symbol('BatchRelation')

function config(factory, value, keys) {
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError(`${factory} expects a configuration object`)
  const unknown = Reflect.ownKeys(value).filter((key) => typeof key !== 'string' || !keys.has(key))
  if (unknown.length)
    throw new TypeError(
      `${factory} received unknown configuration ${unknown.length === 1 ? 'field' : 'fields'} ${unknown.map(String).join(', ')}`,
    )
  return value
}

function metadata(graph, label) {
  const value = graph && graph.compositionMetadata
  if (!value || !Array.isArray(value.fields) || !Array.isArray(value.parameters))
    throw new TypeError(`${label} must be a RelationalQueryGraph`)
  return value
}

function compositionError(issues) {
  const error = new TypeError(
    `Invalid composed query graph:\n${issues.map((issue) => `- ${issue.code} at ${issue.location}: ${issue.message}`).join('\n')}`,
  )
  error.name = 'QueryGraphError'
  error.code = 'QUERY_GRAPH_COMPOSITION_INVALID'
  error.phase = 'composition'
  error.issues = issues
  return error
}

function operationError(issues) {
  const error = new TypeError(
    `Invalid composed query operation:\n${issues.map((issue) => `- ${issue.code} at ${issue.location}: ${issue.message}`).join('\n')}`,
  )
  error.name = 'QueryGraphError'
  error.code = 'QUERY_GRAPH_OPERATION_INVALID'
  error.phase = 'operation'
  error.issues = issues
  return error
}

function batchRelation(value) {
  const relation = config('batchRelation', value, BATCH_KEYS)
  for (const key of ['name', 'from', 'graph', 'to', 'parameter', 'cardinality']) {
    if (relation[key] === undefined) throw new TypeError(`batchRelation requires ${key}`)
  }
  if (relation.cardinality !== 'one' && relation.cardinality !== 'many')
    throw new TypeError("batchRelation cardinality must be 'one' or 'many'")
  metadata(relation.graph, 'batchRelation.graph')
  const parameter =
    typeof relation.parameter === 'string' ? relation.parameter : relation.parameter && relation.parameter.name
  if (typeof parameter !== 'string')
    throw new TypeError('batchRelation.parameter must be a list parameter reference or name')
  return Object.freeze({
    ...relation,
    parameter,
    parameters: Object.freeze({ ...(relation.parameters || {}) }),
    [batchBrand]: true,
  })
}

class ComposedQueryGraph {
  constructor(value) {
    const definition = config('composeGraph', value, COMPOSE_KEYS)
    const rootMetadata = metadata(definition.root, 'composeGraph.root')
    if (!Array.isArray(definition.relations)) throw new TypeError('composeGraph.relations must be an array')
    const issues = []
    const names = new Set()
    for (const [index, relation] of definition.relations.entries()) {
      const location = `relations[${index}]`
      if (!relation || relation[batchBrand] !== true) {
        issues.push({ code: 'invalidBatchRelation', location, message: 'relation must be created by batchRelation' })
        continue
      }
      if (names.has(relation.name))
        issues.push({
          code: 'duplicateRelationName',
          location: `${location}.name`,
          message: `relation ${JSON.stringify(relation.name)} is defined more than once`,
        })
      names.add(relation.name)
      const childMetadata = metadata(relation.graph, `${location}.graph`)
      const parent = rootMetadata.fields.find((field) => field.path === relation.from)
      const child = childMetadata.fields.find((field) => field.path === relation.to)
      const parameter = childMetadata.parameters.find((item) => item.name === relation.parameter)
      if (!parent)
        issues.push({
          code: 'unknownParentKey',
          location: `${location}.from`,
          message: `projection field ${JSON.stringify(relation.from)} is not defined`,
        })
      if (!child)
        issues.push({
          code: 'unknownChildKey',
          location: `${location}.to`,
          message: `projection field ${JSON.stringify(relation.to)} is not defined`,
        })
      if (!parameter)
        issues.push({
          code: 'unknownKeyParameter',
          location: `${location}.parameter`,
          message: `parameter ${JSON.stringify(relation.parameter)} is not defined`,
        })
      else if (parameter.shape !== 'list')
        issues.push({
          code: 'keyParameterNotList',
          location: `${location}.parameter`,
          message: 'batch key parameter must have list shape',
        })
      if (
        parent &&
        child &&
        parameter &&
        new Set([parent.scalarType, child.scalarType, parameter.scalarType]).size !== 1
      )
        issues.push({
          code: 'incompatibleKeyTypes',
          location,
          message: 'parent key, child key, and key parameter must have the same scalar type',
        })
      const statics = relation.parameters || {}
      for (const name of Object.keys(statics)) {
        if (name === relation.parameter)
          issues.push({
            code: 'keyParameterIsStatic',
            location: `${location}.parameters.${name}`,
            message: 'the batch key parameter is supplied automatically',
          })
        else if (!childMetadata.parameters.some((item) => item.name === name))
          issues.push({
            code: 'unknownStaticParameter',
            location: `${location}.parameters.${name}`,
            message: `parameter ${JSON.stringify(name)} is not defined`,
          })
      }
      for (const item of childMetadata.parameters)
        if (item.required && item.name !== relation.parameter && !Object.hasOwn(statics, item.name))
          issues.push({
            code: 'missingStaticParameter',
            location: `${location}.parameters.${item.name}`,
            message: `required child parameter ${JSON.stringify(item.name)} must be supplied statically`,
          })
    }
    if (issues.length) throw compositionError(issues)
    this.rootGraph = definition.root
    this.relations = Object.freeze([...definition.relations])
    this.rootMetadata = rootMetadata
  }

  compileOraclePlan(operation, options) {
    return this.#compile('oracle', operation, options)
  }
  compileSqlServerPlan(operation, options) {
    return this.#compile('sqlServer', operation, options)
  }

  #compile(dialect, operation = {}, options) {
    const selected = operation.select === undefined ? undefined : [...operation.select]
    const rootSelect = []
    const childSelect = new Map()
    const issues = []
    const seen = new Set()
    if (selected)
      for (const [index, path] of selected.entries()) {
        if (seen.has(path)) {
          issues.push({
            code: 'duplicateSelection',
            location: `select[${index}]`,
            message: `projection field ${JSON.stringify(path)} is selected more than once`,
          })
          continue
        }
        seen.add(path)
        const dot = path.indexOf('.')
        if (dot < 0) {
          if (!this.rootMetadata.fields.some((field) => field.path === path))
            issues.push({
              code: 'unknownSelection',
              location: `select[${index}]`,
              message: `projection field ${JSON.stringify(path)} is not defined`,
            })
          else rootSelect.push(path)
        } else {
          const name = path.slice(0, dot),
            childPath = path.slice(dot + 1)
          const relation = this.relations.find((item) => item.name === name)
          const valid = relation && metadata(relation.graph, '').fields.some((field) => field.path === childPath)
          if (!valid)
            issues.push({
              code: 'unknownSelection',
              location: `select[${index}]`,
              message: `projection field ${JSON.stringify(path)} is not defined`,
            })
          else (childSelect.get(name) || (childSelect.set(name, []), childSelect.get(name))).push(childPath)
        }
      }
    if (issues.length) throw operationError(issues)
    const batches = []
    for (const relation of this.relations) {
      const paths = childSelect.get(relation.name)
      if (!paths) continue
      const parentKeyInjected = !rootSelect.includes(relation.from)
      const childKeyInjected = !paths.includes(relation.to)
      if (parentKeyInjected) rootSelect.push(relation.from)
      if (childKeyInjected) paths.push(relation.to)
      batches.push(
        Object.freeze({
          name: relation.name,
          parentKey: relation.from,
          childKey: relation.to,
          cardinality: relation.cardinality,
          parentKeyInjected,
          childKeyInjected,
        }),
      )
    }
    const rootOperation = { ...operation }
    if (selected) rootOperation.select = rootSelect
    const compile = dialect === 'oracle' ? 'compileOracle' : 'compileSqlServer'
    const root = this.rootGraph[compile](rootOperation, options)
    return new CompiledQueryPlan(root, batches, this.relations, childSelect, compile, options)
  }
}

class CompiledQueryPlan {
  constructor(root, batches, relations, selections, compiler, options) {
    this.root = root
    this.batches = Object.freeze(batches)
    this._relations = relations
    this._selections = selections
    this._compiler = compiler
    this._options = options
  }
  compileBatch(name, keys) {
    const relation = this._relations.find((item) => item.name === name)
    if (!relation || !this._selections.has(name))
      throw new RangeError(`Unknown selected batch relation ${JSON.stringify(name)}`)
    if (!Array.isArray(keys)) throw new TypeError('compileBatch keys must be an array')
    return relation.graph[this._compiler](
      {
        select: this._selections.get(name),
        ordering: relation.ordering,
        parameters: { ...relation.parameters, [relation.parameter]: keys },
      },
      this._options,
    )
  }
}

function composeGraph(value) {
  return new ComposedQueryGraph(value)
}

module.exports = { batchRelation, composeGraph, ComposedQueryGraph, CompiledQueryPlan }
