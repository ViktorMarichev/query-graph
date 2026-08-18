'use strict'

const native = require('./native.js')
const { batchQuery, batchRelation, composeGraph } = require('./dsl.js')

exports.CompiledQueryPlan = native.CompiledQueryPlan
exports.ComposedQueryGraph = native.ComposedQueryGraph
exports.QueryGraph = native.QueryGraph
exports.RelationalQueryGraph = native.RelationalQueryGraph
exports.registerDefinition = native.registerDefinition
exports.batchQuery = batchQuery
exports.batchRelation = batchRelation
exports.composeGraph = composeGraph
