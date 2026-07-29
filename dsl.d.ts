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

export type JsonValue = null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue }

export type ScalarParameterValue<Type extends ScalarType> = Type extends 'boolean'
  ? boolean
  : Type extends 'int32' | 'float64'
    ? number
    : Type extends 'int64' | 'decimal'
      ? number | string
      : Type extends 'string' | 'date' | 'dateTime' | 'binary'
        ? string
        : Type extends 'json'
          ? JsonValue
          : never

export type ParameterShape = 'scalar' | 'list'
export type RelationCardinality = 'one' | 'many'
export type OrderDirection = 'asc' | 'desc'
export type NullsOrder = 'first' | 'last'
export type SemanticFunctionName = 'lower' | 'upper' | 'coalesce' | 'concat'
export type AggregateFunctionName = 'count' | 'countDistinct' | 'sum' | 'average' | 'minimum' | 'maximum'
export type ProjectionFieldRole = 'value' | 'dimension' | 'measure'

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
  shape?: ParameterShape
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

export interface InParameterExpression<Name extends string = string> {
  kind: 'inParameter'
  expression: Expression
  parameter: Name
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

export interface AggregateExpression {
  kind: 'aggregate'
  function: AggregateFunctionName
  expression?: Expression
}

export type Expression =
  | FieldExpression
  | ParameterExpression
  | LiteralExpression
  | BinaryExpression
  | LikeExpression
  | InExpression
  | InParameterExpression
  | ExpressionGroup
  | UnaryExpression
  | ExistsExpression
  | FunctionExpression
  | AggregateExpression

export interface RelationDefinition {
  name: string
  from: string
  to: string
  cardinality?: RelationCardinality
  required?: boolean
  selection?: RelationSelection
  on: Expression
}

export type ConstraintCondition = { kind: 'always' } | { kind: 'parameterPresent'; parameter: string }

export interface ConstraintDefinition {
  when?: ConstraintCondition
  predicate: Expression
}

export type JoinProjectionPath<Segments extends readonly string[]> = number extends Segments['length']
  ? string
  : Segments extends readonly []
    ? ''
    : Segments extends readonly [infer Only extends string]
      ? Only
      : Segments extends readonly [infer Head extends string, ...infer Tail extends readonly string[]]
        ? `${Head}.${JoinProjectionPath<Tail>}`
        : string

declare const projectionPathType: unique symbol
declare const summaryFieldType: unique symbol

export interface ProjectionFieldDefinition<Path extends string = string> {
  path: string[]
  readonly [projectionPathType]?: Path
  expression: Expression
  role?: ProjectionFieldRole
  selectedByDefault?: boolean
}

export interface DimensionDefinition<Path extends string = string> extends ProjectionFieldDefinition<Path> {
  readonly [summaryFieldType]: 'dimension'
  role: 'dimension'
}

export interface MeasureDefinition<Path extends string = string> extends ProjectionFieldDefinition<Path> {
  readonly [summaryFieldType]: 'measure'
  role: 'measure'
}

export type SummaryFieldDefinition<Path extends string = string> = DimensionDefinition<Path> | MeasureDefinition<Path>

export interface ProjectionDefinition<Path extends string = string> {
  fields: ProjectionFieldDefinition<Path>[]
}

export interface ProjectionDefinitionInput<Path extends string = string> {
  fields?: ProjectionFieldDefinition<Path>[]
}

export interface OrderByDefinition {
  expression: Expression
  direction: OrderDirection
  nulls?: NullsOrder
}

export interface OrderingDefinition<Name extends string = string> {
  name: Name
  orderBy: OrderByDefinition[]
  default?: boolean
}

export interface FirstBySelection {
  kind: 'firstBy'
  orderBy: OrderByDefinition[]
}

export type RelationSelection = FirstBySelection

export interface GraphDefinitionInput {
  schemaVersion: 8
  name: string
  root: string
  sources: SourceDefinition[]
  parameters?: ParameterDefinition[]
  relations?: RelationDefinition[]
  constraints?: ConstraintDefinition[]
  projection?: ProjectionDefinitionInput
  orderings?: OrderingDefinition[]
}

export interface GraphDefinition<
  Parameter extends ParameterDefinition = ParameterDefinition,
  ProjectionPath extends string = string,
  OrderingName extends string = string,
> extends GraphDefinitionInput {
  parameters: Parameter[]
  relations: RelationDefinition[]
  constraints: ConstraintDefinition[]
  projection: ProjectionDefinition<ProjectionPath>
  orderings: OrderingDefinition<OrderingName>[]
}

export type ExactGraphDefinitionInput<Definition extends GraphDefinitionInput> = Definition &
  Record<Exclude<keyof Definition, keyof GraphDefinitionInput>, never>

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

export interface QueryOperationBase<SelectPath extends string = string, OrderingName extends string = string> {
  select?: readonly SelectPath[]
  ordering?: OrderingName
  offset?: number
  limit?: number
}

export type DefinitionParameter<Definition extends GraphDefinitionInput> =
  Definition extends GraphDefinition<infer Parameter, infer _ProjectionPath, infer _OrderingName>
    ? Parameter
    : NonNullable<Definition['parameters']>[number]

export type DefinitionProjectionPath<Definition extends GraphDefinitionInput> =
  Definition extends GraphDefinition<infer _Parameter, infer ProjectionPath, infer _OrderingName>
    ? ProjectionPath
    : Definition['projection'] extends ProjectionDefinitionInput<infer ProjectionPath>
      ? ProjectionPath
      : string

export type DefinitionOrderingName<Definition extends GraphDefinitionInput> =
  Definition extends GraphDefinition<infer _Parameter, infer _ProjectionPath, infer OrderingName>
    ? OrderingName
    : NonNullable<Definition['orderings']>[number] extends OrderingDefinition<infer OrderingName>
      ? OrderingName
      : never

export type ParameterValue<Parameter extends ParameterDefinition> = Parameter extends { shape: 'list' }
  ? readonly ScalarParameterValue<Parameter['scalarType']>[]
  : ScalarParameterValue<Parameter['scalarType']>

type RequiredParameter<Parameter extends ParameterDefinition> = Parameter extends unknown
  ? Parameter extends { required?: true }
    ? Parameter
    : never
  : never

type OptionalParameter<Parameter extends ParameterDefinition> = Parameter extends unknown
  ? Parameter extends { required?: true }
    ? never
    : Parameter
  : never

export type OperationParameters<Definition extends GraphDefinitionInput> = {
  [Parameter in RequiredParameter<DefinitionParameter<Definition>> as Parameter['name']]-?: ParameterValue<Parameter>
} & {
  [Parameter in OptionalParameter<DefinitionParameter<Definition>> as Parameter['name']]?: ParameterValue<Parameter>
}

type OperationParameterInput<Definition extends GraphDefinitionInput> = [DefinitionParameter<Definition>] extends [
  never,
]
  ? { parameters?: never }
  : [RequiredParameter<DefinitionParameter<Definition>>] extends [never]
    ? { parameters?: OperationParameters<Definition> }
    : { parameters: OperationParameters<Definition> }

export type QueryOperation<Definition extends GraphDefinitionInput = GraphDefinitionInput> = QueryOperationBase<
  DefinitionProjectionPath<Definition>,
  DefinitionOrderingName<Definition>
> &
  OperationParameterInput<Definition>

export interface QueryGraph<Definition extends GraphDefinitionInput = GraphDefinitionInput> {
  readonly name: string
  readonly root: string
  readonly sourceCount: number
  readonly relationCount: number
  hasSource(source: string): boolean
  hasField(source: string, field: string): boolean
  hasParameter(parameter: string): boolean
  hasRelation(relation: string): boolean
  selectableFields(): Array<DefinitionProjectionPath<Definition>>
  withRelationalMapping(mapping: RelationalMapping): RelationalQueryGraph<Definition>
}

export interface RelationalQueryGraph<Definition extends GraphDefinitionInput = GraphDefinitionInput> {
  readonly name: string
  compileSqlServer(
    operation: QueryOperation<Definition>,
    options?: SqlServerCompileOptions,
  ): import('./index.js').CompiledSqlStatement
  compileOracle(
    operation: QueryOperation<Definition>,
    options?: OracleCompileOptions,
  ): import('./index.js').CompiledSqlStatement
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

export interface ScalarParameterRef<
  Name extends string = string,
  Type extends ScalarType = ScalarType,
  Required extends boolean = boolean,
> extends ParameterDefinition {
  name: Name
  scalarType: Type
  shape?: 'scalar'
  required?: Required
}

export interface ListParameterRef<
  Name extends string = string,
  Type extends ScalarType = ScalarType,
  Required extends boolean = boolean,
> extends ParameterDefinition {
  name: Name
  scalarType: Type
  shape: 'list'
  required?: Required
}

export type ParameterRef<
  Name extends string = string,
  Type extends ScalarType = ScalarType,
  Required extends boolean = boolean,
> = ScalarParameterRef<Name, Type, Required> | ListParameterRef<Name, Type, Required>

export interface RelationRef<Name extends string = string> extends RelationDefinition {
  name: Name
}

export type LiteralInput = null | boolean | string | number
export type ExpressionInput = Expression | LiteralInput | SummaryFieldDefinition

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

export function requiredParameter<const Name extends string, const Type extends ScalarType>(
  name: Name,
  scalarType: Type,
): ScalarParameterRef<Name, Type, true>
export function optionalParameter<const Name extends string, const Type extends ScalarType>(
  name: Name,
  scalarType: Type,
): ScalarParameterRef<Name, Type, false>
export function requiredListParameter<const Name extends string, const Type extends ScalarType>(
  name: Name,
  scalarType: Type,
): ListParameterRef<Name, Type, true>
export function optionalListParameter<const Name extends string, const Type extends ScalarType>(
  name: Name,
  scalarType: Type,
): ListParameterRef<Name, Type, false>
export function param<const Name extends string, const Type extends ScalarType>(
  parameter: ScalarParameterRef<Name, Type>,
): ParameterExpression<Name>

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
export function inParameter<const Name extends string, const Type extends ScalarType>(
  expression: ExpressionInput,
  parameter: ListParameterRef<Name, Type>,
): InParameterExpression<Name>
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

export function count(expression?: ExpressionInput): AggregateExpression
export function countDistinct(expression: ExpressionInput): AggregateExpression
export function sum(expression: ExpressionInput): AggregateExpression
export function average(expression: ExpressionInput): AggregateExpression
export function minimum(expression: ExpressionInput): AggregateExpression
export function maximum(expression: ExpressionInput): AggregateExpression

export interface RelationConfiguration<Name extends string = string> {
  name: Name
  from: string | SourceRef
  to: string | SourceRef
  on: Expression
  required?: boolean
  cardinality?: RelationCardinality
  selection?: RelationSelection
}

export function relation<const Name extends string>(configuration: RelationConfiguration<Name>): RelationRef<Name>

export interface ConstraintConfiguration {
  predicate: Expression
  when?: string | ParameterRef
}

export function constraint(configuration: ConstraintConfiguration): ConstraintDefinition

export interface ProjectionConfiguration<Path extends string | readonly string[] = string | readonly string[]> {
  path: Path
  expression: ExpressionInput
  default?: boolean
}

type ConfigurationPath<Path extends string | readonly string[]> = Path extends string
  ? Path
  : Path extends readonly string[]
    ? JoinProjectionPath<Path>
    : never

export function project<const Path extends string | readonly string[]>(
  configuration: ProjectionConfiguration<Path>,
): ProjectionFieldDefinition<ConfigurationPath<Path>>

export function dimension<const Path extends string | readonly string[]>(
  configuration: ProjectionConfiguration<Path>,
): DimensionDefinition<ConfigurationPath<Path>>

export function measure<const Path extends string | readonly string[]>(
  configuration: ProjectionConfiguration<Path>,
): MeasureDefinition<ConfigurationPath<Path>>

export interface OrderByOptions {
  nulls?: NullsOrder
}

export function asc(expression: ExpressionInput, options?: OrderByOptions): OrderByDefinition
export function desc(expression: ExpressionInput, options?: OrderByOptions): OrderByDefinition

export function firstBy(firstOrder: OrderByDefinition, ...rest: readonly OrderByDefinition[]): FirstBySelection

export interface OrderingConfiguration<Name extends string = string> {
  name: Name
  by: readonly [OrderByDefinition, ...OrderByDefinition[]]
  default?: boolean
}

export function ordering<const Name extends string>(
  configuration: OrderingConfiguration<Name>,
): OrderingDefinition<Name>

export interface GraphModuleConfiguration {
  name: string
  modules?: readonly GraphModule[]
  sources?: readonly SourceRef[]
  parameters?: readonly ParameterDefinition[]
  relations?: readonly RelationDefinition[]
  constraints?: readonly ConstraintDefinition[]
  projection?: readonly ProjectionFieldDefinition[]
  orderings?: readonly OrderingDefinition[]
}

type ConfigurationElement<Configuration, Key extends PropertyKey> = Key extends keyof Configuration
  ? NonNullable<Configuration[Key]> extends readonly (infer Element)[]
    ? Element
    : never
  : never

type ModuleParameter<Module> =
  Module extends GraphModule<infer Parameter, infer _ProjectionPath, infer _OrderingName> ? Parameter : never

type ModuleProjectionPath<Module> =
  Module extends GraphModule<infer _Parameter, infer ProjectionPath, infer _OrderingName> ? ProjectionPath : never

type ModuleOrderingName<Module> =
  Module extends GraphModule<infer _Parameter, infer _ProjectionPath, infer OrderingName> ? OrderingName : never

type ProjectionPathOf<Field> = Field extends ProjectionFieldDefinition<infer Path> ? Path : never

type OrderingNameOf<Ordering> = Ordering extends OrderingDefinition<infer Name> ? Name : never

type ConfigurationParameter<Configuration> =
  | Extract<ConfigurationElement<Configuration, 'parameters'>, ParameterDefinition>
  | ModuleParameter<ConfigurationElement<Configuration, 'modules'>>

type ConfigurationProjectionPath<Configuration> =
  | ProjectionPathOf<ConfigurationElement<Configuration, 'projection'>>
  | ProjectionPathOf<ConfigurationElement<Configuration, 'dimensions'>>
  | ProjectionPathOf<ConfigurationElement<Configuration, 'measures'>>
  | ModuleProjectionPath<ConfigurationElement<Configuration, 'modules'>>

type ConfigurationOrderingName<Configuration> =
  | OrderingNameOf<ConfigurationElement<Configuration, 'orderings'>>
  | ModuleOrderingName<ConfigurationElement<Configuration, 'modules'>>

export interface GraphModule<
  Parameter extends ParameterDefinition = ParameterDefinition,
  ProjectionPath extends string = string,
  OrderingName extends string = string,
> {
  readonly name: string
  readonly sources: readonly SourceRef[]
  readonly parameters: readonly Parameter[]
  readonly relations: readonly RelationDefinition[]
  readonly constraints: readonly ConstraintDefinition[]
  readonly projection: readonly ProjectionFieldDefinition<ProjectionPath>[]
  readonly orderings: readonly OrderingDefinition<OrderingName>[]
}

export function defineGraphModule<const Configuration extends GraphModuleConfiguration>(
  configuration: Configuration,
): GraphModule<
  ConfigurationParameter<Configuration>,
  ConfigurationProjectionPath<Configuration>,
  ConfigurationOrderingName<Configuration>
>

export interface GraphConfiguration {
  name: string
  root: string | SourceRef
  modules?: readonly GraphModule[]
  sources?: readonly SourceRef[]
  parameters?: readonly ParameterDefinition[]
  relations?: readonly RelationDefinition[]
  constraints?: readonly ConstraintDefinition[]
  projection?: readonly ProjectionFieldDefinition[]
  orderings?: readonly OrderingDefinition[]
}

export function defineGraph<const Configuration extends GraphConfiguration>(
  configuration: Configuration,
): GraphDefinition<
  ConfigurationParameter<Configuration>,
  ConfigurationProjectionPath<Configuration>,
  ConfigurationOrderingName<Configuration>
>

export interface SummaryGraphConfiguration {
  name: string
  root: string | SourceRef
  modules?: readonly GraphModule[]
  sources?: readonly SourceRef[]
  parameters?: readonly ParameterDefinition[]
  relations?: readonly RelationDefinition[]
  constraints?: readonly ConstraintDefinition[]
  dimensions?: readonly DimensionDefinition[]
  measures?: readonly MeasureDefinition[]
  orderings?: readonly OrderingDefinition[]
}

export function defineSummaryGraph<const Configuration extends SummaryGraphConfiguration>(
  configuration: Configuration,
): GraphDefinition<
  ConfigurationParameter<Configuration>,
  ConfigurationProjectionPath<Configuration>,
  ConfigurationOrderingName<Configuration>
>
