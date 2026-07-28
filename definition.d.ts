export const GRAPH_DEFINITION_VERSION: 4

export type ScalarType =
  | 'boolean'
  | 'int32'
  | 'int64'
  | 'float64'
  | 'decimal'
  | 'string'
  | 'date'
  | 'dateTime'
  | 'binary'
  | 'json'

export type RelationCardinality = 'one' | 'many'
export type OrderDirection = 'asc' | 'desc'
export type NullsOrder = 'first' | 'last'
export type SemanticFunctionName = 'lower' | 'upper' | 'coalesce' | 'concat'

export interface FieldDefinition {
  name: string
  scalarType: ScalarType
  nullable?: boolean
  selectable?: boolean
}

export interface SourceDefinition {
  key: string
  fields: FieldDefinition[]
}

export interface ParameterDefinition {
  name: string
  scalarType: ScalarType
  required?: boolean
}

export interface FieldExpression<Source extends string = string, Field extends string = string> {
  kind: 'field'
  source: Source
  field: Field
}

export interface ParameterExpression<Name extends string = string> {
  kind: 'parameter'
  name: Name
}

export type LiteralValue =
  | { kind: 'null' }
  | { kind: 'boolean'; value: boolean }
  | { kind: 'integer'; value: number }
  | { kind: 'decimal'; value: string }
  | { kind: 'string'; value: string }

export interface LiteralExpression {
  kind: 'literal'
  value: LiteralValue
}

export type BinaryExpressionKind =
  | 'eq'
  | 'notEq'
  | 'lessThan'
  | 'lessThanOrEqual'
  | 'greaterThan'
  | 'greaterThanOrEqual'

export interface BinaryExpression {
  kind: BinaryExpressionKind
  left: Expression
  right: Expression
}

export interface LikeExpression {
  kind: 'like'
  expression: Expression
  pattern: Expression
}

export interface InExpression {
  kind: 'in'
  expression: Expression
  values: Expression[]
}

export interface ExpressionGroup {
  kind: 'and' | 'or'
  expressions: Expression[]
}

export interface UnaryExpression {
  kind: 'not' | 'isNull' | 'isNotNull'
  expression: Expression
}

export interface ExistsExpression<Source extends string = string> {
  kind: 'exists'
  source: Source
  predicate?: Expression
}

export interface FunctionExpression {
  kind: 'function'
  name: SemanticFunctionName
  arguments: Expression[]
}

export type Expression =
  | FieldExpression
  | ParameterExpression
  | LiteralExpression
  | BinaryExpression
  | LikeExpression
  | InExpression
  | ExpressionGroup
  | UnaryExpression
  | ExistsExpression
  | FunctionExpression

export interface RelationDefinition {
  name: string
  from: string
  to: string
  cardinality?: RelationCardinality
  required?: boolean
  on: Expression
}

export type ConstraintCondition = { kind: 'always' } | { kind: 'parameterPresent'; parameter: string }

export interface ConstraintDefinition {
  name: string
  when?: ConstraintCondition
  predicate: Expression
}

export interface ProjectionFieldDefinition {
  path: string[]
  expression: Expression
  selectedByDefault?: boolean
}

export interface ProjectionDefinition {
  fields: ProjectionFieldDefinition[]
}

export interface ProjectionDefinitionInput {
  fields?: ProjectionFieldDefinition[]
}

export interface OrderByDefinition {
  expression: Expression
  direction: OrderDirection
  nulls?: NullsOrder
}

export interface GraphDefinitionInput {
  schemaVersion: typeof GRAPH_DEFINITION_VERSION
  name: string
  root: string
  sources: SourceDefinition[]
  parameters?: ParameterDefinition[]
  relations?: RelationDefinition[]
  constraints?: ConstraintDefinition[]
  projection?: ProjectionDefinitionInput
  defaultOrderBy?: OrderByDefinition[]
}

export interface GraphDefinition extends GraphDefinitionInput {
  parameters: ParameterDefinition[]
  relations: RelationDefinition[]
  constraints: ConstraintDefinition[]
  projection: ProjectionDefinition
  defaultOrderBy: OrderByDefinition[]
}

export type TableName =
  | string
  | {
      catalog?: string
      schema?: string
      name: string
    }

export interface SourceMapping {
  table: TableName
  columns?: Record<string, string>
}

export interface RelationalMapping {
  sources: Record<string, SourceMapping>
}

export interface QueryOperation {
  select?: string[]
  parameters?: Record<string, unknown>
  offset?: number
  limit?: number
}

export type SqlServerVersion = '2008' | '2012' | '2016' | '2019' | '2022'
export type OracleVersion = '11g' | '12c' | '19c' | '21c' | '23ai'

export interface SqlServerCompileOptions {
  version?: SqlServerVersion
}

export interface OracleCompileOptions {
  version?: OracleVersion
}

export type QueryGraphErrorPhase = 'definition' | 'mapping' | 'operation' | 'sql'

export interface QueryGraphDiagnostic {
  code: string
  location: string
  message: string
}

export interface QueryGraphError extends Error {
  readonly name: 'QueryGraphError'
  readonly code: string
  readonly phase: QueryGraphErrorPhase
  readonly issues: readonly QueryGraphDiagnostic[]
}

export interface FieldSpecDefinition {
  scalarType: ScalarType
  nullable?: boolean
  selectable?: boolean
}

export type FieldSpec = ScalarType | FieldSpecDefinition
export type FieldSpecMap = Record<string, FieldSpec>

export interface SourceRef<Key extends string = string, Fields extends FieldSpecMap = FieldSpecMap>
  extends SourceDefinition {
  key: Key
  field<Name extends Extract<keyof Fields, string>>(name: Name): FieldExpression<Key, Name>
}

export interface ParameterRef<Name extends string = string> extends ParameterDefinition {
  name: Name
}

export interface RelationRef<Name extends string = string> extends RelationDefinition {
  name: Name
}

export type LiteralInput = null | boolean | string | number
export type ExpressionInput = Expression | LiteralInput

export interface FieldTypeOptions {
  nullable?: boolean
  selectable?: boolean
}

export function fieldType(scalarType: ScalarType, options?: FieldTypeOptions): FieldSpecDefinition
export function nullable(specification: FieldSpec): FieldSpecDefinition
export function hidden(specification: FieldSpec): FieldSpecDefinition

export function source<const Key extends string, const Fields extends FieldSpecMap>(
  key: Key,
  fields: Fields,
): SourceRef<Key, Fields>

export function field<
  const Key extends string,
  const Fields extends FieldSpecMap,
  const Name extends Extract<keyof Fields, string>,
>(source: SourceRef<Key, Fields>, name: Name): FieldExpression<Key, Name>
export function field<const Source extends string, const Name extends string>(
  source: Source,
  name: Name,
): FieldExpression<Source, Name>

export function requiredParameter<const Name extends string>(name: Name, scalarType: ScalarType): ParameterRef<Name>
export function optionalParameter<const Name extends string>(name: Name, scalarType: ScalarType): ParameterRef<Name>
export function param<const Name extends string>(parameter: Name | ParameterRef<Name>): ParameterExpression<Name>

export function literal(value: LiteralInput): LiteralExpression
export function integer(value: number): LiteralExpression
export function decimal(value: string | number): LiteralExpression

export function eq(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function neq(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function lt(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function lte(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function gt(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function gte(left: ExpressionInput, right: ExpressionInput): BinaryExpression
export function like(expression: ExpressionInput, pattern: ExpressionInput): LikeExpression
export function inList(expression: ExpressionInput, values: readonly ExpressionInput[]): InExpression
export function and(...expressions: readonly ExpressionInput[]): ExpressionGroup
export function or(...expressions: readonly ExpressionInput[]): ExpressionGroup
export function not(expression: ExpressionInput): UnaryExpression
export function isNull(expression: ExpressionInput): UnaryExpression
export function isNotNull(expression: ExpressionInput): UnaryExpression
export function exists<const Source extends string>(
  source: Source | SourceRef<Source>,
  predicate?: ExpressionInput,
): ExistsExpression<Source>
export function lower(expression: ExpressionInput): FunctionExpression
export function upper(expression: ExpressionInput): FunctionExpression
export function coalesce(
  first: ExpressionInput,
  second: ExpressionInput,
  ...rest: readonly ExpressionInput[]
): FunctionExpression
export function concat(first: ExpressionInput, ...rest: readonly ExpressionInput[]): FunctionExpression

export interface RelationOptions {
  required?: boolean
  cardinality?: RelationCardinality
}

export function relation<const Name extends string>(
  name: Name,
  from: string | SourceRef,
  to: string | SourceRef,
  on: Expression,
  options?: RelationOptions,
): RelationRef<Name>

export interface ConstraintOptions {
  when?: string | ParameterRef
}

export function constraint(name: string, predicate: Expression, options?: ConstraintOptions): ConstraintDefinition

export interface ProjectionOptions {
  default?: boolean
}

export function project(
  path: string | readonly string[],
  expression: ExpressionInput,
  options?: ProjectionOptions,
): ProjectionFieldDefinition

export interface OrderByOptions {
  nulls?: NullsOrder
}

export function asc(expression: ExpressionInput, options?: OrderByOptions): OrderByDefinition
export function desc(expression: ExpressionInput, options?: OrderByOptions): OrderByDefinition

export interface GraphModuleConfiguration {
  name: string
  modules?: readonly GraphModule[]
  sources?: readonly SourceRef[]
  parameters?: readonly ParameterDefinition[]
  relations?: readonly RelationDefinition[]
  constraints?: readonly ConstraintDefinition[]
  projection?: readonly ProjectionFieldDefinition[]
  defaultOrderBy?: readonly OrderByDefinition[]
}

export interface GraphModule {
  readonly name: string
  readonly sources: readonly SourceRef[]
  readonly parameters: readonly ParameterDefinition[]
  readonly relations: readonly RelationDefinition[]
  readonly constraints: readonly ConstraintDefinition[]
  readonly projection: readonly ProjectionFieldDefinition[]
  readonly defaultOrderBy: readonly OrderByDefinition[]
}

export function defineGraphModule(configuration: GraphModuleConfiguration): GraphModule

export interface GraphConfiguration {
  name: string
  root: string | SourceRef
  modules?: readonly GraphModule[]
  sources?: readonly SourceRef[]
  parameters?: readonly ParameterDefinition[]
  relations?: readonly RelationDefinition[]
  constraints?: readonly ConstraintDefinition[]
  projection?: readonly ProjectionFieldDefinition[]
  defaultOrderBy?: readonly OrderByDefinition[]
}

export function defineGraph(configuration: GraphConfiguration): GraphDefinition
