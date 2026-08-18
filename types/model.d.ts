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

type BooleanOr<Value extends boolean> = true extends Value ? true : false

export interface SourceFieldNullability<Source extends string = string, FieldNullable extends boolean = boolean> {
  kind: 'sourceField'
  source: Source
  fieldNullable: FieldNullable
}

export interface AnyNullability<Values = unknown> {
  kind: 'any'
  values: Values
}

export interface AllNullability<Values = unknown> {
  kind: 'all'
  values: Values
}

export type NullabilityExpression = boolean | SourceFieldNullability | AnyNullability | AllNullability

declare const expressionTypeMetadata: unique symbol

export interface ExpressionTypeMetadata<
  Type extends ScalarType | null = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
> {
  readonly [expressionTypeMetadata]: {
    scalarType: Type
    nullability: Nullability
  }
}

export type TypedExpression<
  Definition extends Expression = Expression,
  Type extends ScalarType | null = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
> = Definition & ExpressionTypeMetadata<Type, Nullability>

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

export interface ExistsExpression<Source extends string = string, From extends string = string> {
  kind: 'exists'
  source: Source
  from?: From
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

export interface RelationDefinition<
  Name extends string = string,
  From extends string = string,
  To extends string = string,
  Required extends boolean = boolean,
  Cardinality extends RelationCardinality = RelationCardinality,
> {
  name: Name
  from: From
  to: To
  cardinality?: Cardinality
  required?: Required
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
declare const projectionScalarType: unique symbol
declare const projectionNullabilityType: unique symbol
declare const projectionDefaultType: unique symbol
declare const projectionObjectPathType: unique symbol
declare const projectionObjectNullabilityType: unique symbol
declare const summaryFieldType: unique symbol
declare const typedProjectionPathType: unique symbol
declare const typedProjectionScalarType: unique symbol
declare const typedProjectionNullabilityType: unique symbol
declare const typedProjectionDefaultType: unique symbol
declare const typedProjectionObjectPathType: unique symbol
declare const typedProjectionObjectNullabilityType: unique symbol

export interface ProjectionTypeMetadata<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> {
  readonly [typedProjectionPathType]: Path
  readonly [typedProjectionScalarType]: Type
  readonly [typedProjectionNullabilityType]: Nullability
  readonly [typedProjectionDefaultType]: SelectedByDefault
}

export interface ProjectionObjectTypeMetadata<
  Path extends string = string,
  Nullability extends NullabilityExpression = NullabilityExpression,
> {
  readonly [typedProjectionObjectPathType]: Path
  readonly [typedProjectionObjectNullabilityType]: Nullability
}

export type TypedProjectionObject<
  Path extends string = string,
  Nullability extends NullabilityExpression = NullabilityExpression,
> = ProjectionObjectDefinition<Path, Nullability> & ProjectionObjectTypeMetadata<Path, Nullability>

export type TypedProjectionField<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> = ProjectionFieldDefinition<Path, Type, Nullability, SelectedByDefault> &
  ProjectionTypeMetadata<Path, Type, Nullability, SelectedByDefault>

export type TypedDimensionDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> = DimensionDefinition<Path, Type, Nullability, SelectedByDefault> &
  ProjectionTypeMetadata<Path, Type, Nullability, SelectedByDefault>

export type TypedMeasureDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> = MeasureDefinition<Path, Type, Nullability, SelectedByDefault> &
  ProjectionTypeMetadata<Path, Type, Nullability, SelectedByDefault>

export interface ProjectionFieldDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> {
  path: string[]
  readonly [projectionPathType]?: Path
  readonly [projectionScalarType]?: Type
  readonly [projectionNullabilityType]?: Nullability
  readonly [projectionDefaultType]?: SelectedByDefault
  expression: Expression
  role?: ProjectionFieldRole
  selectedByDefault?: boolean
}

export interface ProjectionObjectDefinition<
  Path extends string = string,
  Nullability extends NullabilityExpression = NullabilityExpression,
> {
  path: string[]
  readonly [projectionObjectPathType]?: Path
  readonly [projectionObjectNullabilityType]?: Nullability
  presence: Expression
}

export interface DimensionDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> extends ProjectionFieldDefinition<Path, Type, Nullability, SelectedByDefault> {
  readonly [summaryFieldType]: 'dimension'
  role: 'dimension'
}

export interface MeasureDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> extends ProjectionFieldDefinition<Path, Type, Nullability, SelectedByDefault> {
  readonly [summaryFieldType]: 'measure'
  role: 'measure'
}

export type SummaryFieldDefinition<
  Path extends string = string,
  Type extends ScalarType = ScalarType,
  Nullability extends NullabilityExpression = NullabilityExpression,
  SelectedByDefault extends boolean = boolean,
> =
  | DimensionDefinition<Path, Type, Nullability, SelectedByDefault>
  | MeasureDefinition<Path, Type, Nullability, SelectedByDefault>

export interface ProjectionDefinition<
  Path extends string = string,
  Field extends ProjectionFieldDefinition = ProjectionFieldDefinition<Path>,
  Object extends ProjectionObjectDefinition = ProjectionObjectDefinition,
> {
  fields: Field[]
  objects: Object[]
}

export interface ProjectionDefinitionInput<
  Path extends string = string,
  Field extends ProjectionFieldDefinition = ProjectionFieldDefinition<Path>,
  Object extends ProjectionObjectDefinition = ProjectionObjectDefinition,
> {
  fields?: Field[]
  objects?: Object[]
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
  schemaVersion: 10
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
  ProjectionField extends ProjectionFieldDefinition = ProjectionFieldDefinition<ProjectionPath>,
  Relation extends RelationDefinition = RelationDefinition,
  Root extends string = string,
  ProjectionObject extends ProjectionObjectDefinition = ProjectionObjectDefinition,
> extends GraphDefinitionInput {
  root: Root
  parameters: Parameter[]
  relations: Relation[]
  constraints: ConstraintDefinition[]
  projection: ProjectionDefinition<ProjectionPath, ProjectionField, ProjectionObject>
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
