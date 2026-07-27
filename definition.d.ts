export const GRAPH_DEFINITION_VERSION: 1

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

export type ParameterCardinality = 'one' | 'many'
export type RelationCardinality = 'one' | 'many'
export type OrderDirection = 'asc' | 'desc'
export type NullsOrder = 'first' | 'last'

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
  cardinality?: ParameterCardinality
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

export interface FunctionExpression {
  kind: 'function'
  name: string
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
  relations?: string[]
  selectable?: boolean
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
  schemaVersion: number
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

export interface ParameterOptions {
  cardinality?: ParameterCardinality
}

export function requiredParameter<const Name extends string>(
  name: Name,
  scalarType: ScalarType,
  options?: ParameterOptions,
): ParameterRef<Name>
export function optionalParameter<const Name extends string>(
  name: Name,
  scalarType: ScalarType,
  options?: ParameterOptions,
): ParameterRef<Name>
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
export function call(name: string, ...arguments_: readonly ExpressionInput[]): FunctionExpression

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
  through?: readonly (string | RelationRef)[]
  selectable?: boolean
  default?: boolean
}

export function project(
  path: string | readonly string[],
  expression: Expression,
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
