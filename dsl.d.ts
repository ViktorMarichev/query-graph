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
declare const summaryFieldType: unique symbol
declare const typedProjectionPathType: unique symbol
declare const typedProjectionScalarType: unique symbol
declare const typedProjectionNullabilityType: unique symbol
declare const typedProjectionDefaultType: unique symbol

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
> {
  fields: Field[]
}

export interface ProjectionDefinitionInput<
  Path extends string = string,
  Field extends ProjectionFieldDefinition = ProjectionFieldDefinition<Path>,
> {
  fields?: Field[]
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
  schemaVersion: 9
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
> extends GraphDefinitionInput {
  root: Root
  parameters: Parameter[]
  relations: Relation[]
  constraints: ConstraintDefinition[]
  projection: ProjectionDefinition<ProjectionPath, ProjectionField>
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
  Definition extends GraphDefinition<
    infer Parameter,
    infer _ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer _Relation,
    infer _Root
  >
    ? Parameter
    : NonNullable<Definition['parameters']>[number]

export type DefinitionProjectionPath<Definition extends GraphDefinitionInput> =
  Definition extends GraphDefinition<
    infer _Parameter,
    infer ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer _Relation,
    infer _Root
  >
    ? ProjectionPath
    : Definition['projection'] extends ProjectionDefinitionInput<infer ProjectionPath>
      ? ProjectionPath
      : string

export type DefinitionOrderingName<Definition extends GraphDefinitionInput> =
  Definition extends GraphDefinition<
    infer _Parameter,
    infer _ProjectionPath,
    infer OrderingName,
    infer _ProjectionField,
    infer _Relation,
    infer _Root
  >
    ? OrderingName
    : NonNullable<Definition['orderings']>[number] extends OrderingDefinition<infer OrderingName>
      ? OrderingName
      : never

export type DefinitionProjectionField<Definition extends GraphDefinitionInput> = NonNullable<
  NonNullable<Definition['projection']>['fields']
>[number]

export type DefinitionRelation<Definition extends GraphDefinitionInput> = NonNullable<Definition['relations']>[number]

export type DefinitionRoot<Definition extends GraphDefinitionInput> = Definition['root']

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

declare const graphDefinitionType: unique symbol

export interface QueryGraph<Definition extends GraphDefinitionInput = GraphDefinitionInput> {
  readonly [graphDefinitionType]?: Definition
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
  readonly [graphDefinitionType]?: Definition
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

export type BatchCardinality = RelationCardinality

export interface BatchRelationWire {
  name: string
  from: string
  to: string
  parameter: string
  cardinality: BatchCardinality
  parameters?: Record<string, unknown>
  ordering?: string
}

type ListDefinitionParameter<Definition extends GraphDefinitionInput> =
  DefinitionParameter<Definition> extends infer Parameter extends ParameterDefinition
    ? Parameter extends { shape: 'list' }
      ? Parameter
      : never
    : never

type ListParameterReference<Definition extends GraphDefinitionInput> =
  | ListDefinitionParameter<Definition>
  | ListDefinitionParameter<Definition>['name']

type ParameterReferenceName<Reference> = Reference extends { name: infer Name extends string }
  ? Name
  : Reference extends string
    ? Reference
    : never

type ResolveListParameter<Definition extends GraphDefinitionInput, Reference> = Extract<
  ListDefinitionParameter<Definition>,
  { name: ParameterReferenceName<Reference> }
>

type BatchStaticParameterCandidate<
  Parameter extends ParameterDefinition,
  KeyName extends string,
> = Parameter extends ParameterDefinition ? (Parameter['name'] extends KeyName ? never : Parameter) : never

type BatchStaticParameter<
  Definition extends GraphDefinitionInput,
  KeyName extends string,
> = BatchStaticParameterCandidate<DefinitionParameter<Definition>, KeyName>

type BatchStaticParameters<Definition extends GraphDefinitionInput, KeyName extends string> = {
  [Parameter in RequiredParameter<
    BatchStaticParameter<Definition, KeyName>
  > as Parameter['name']]-?: ParameterValue<Parameter>
} & {
  [Parameter in OptionalParameter<
    BatchStaticParameter<Definition, KeyName>
  > as Parameter['name']]?: ParameterValue<Parameter>
}

type BatchStaticParameterInput<Definition extends GraphDefinitionInput, KeyName extends string> = [
  BatchStaticParameter<Definition, KeyName>,
] extends [never]
  ? { parameters?: never }
  : [RequiredParameter<BatchStaticParameter<Definition, KeyName>>] extends [never]
    ? { parameters?: BatchStaticParameters<Definition, KeyName> }
    : { parameters: BatchStaticParameters<Definition, KeyName> }

declare const batchRelationType: unique symbol

export interface BatchRelation<
  Name extends string = string,
  Child extends RelationalQueryGraph = RelationalQueryGraph,
  From extends string = string,
  To extends string = string,
  KeyParameter extends ParameterDefinition & { shape: 'list' } = ListParameterRef,
  Cardinality extends BatchCardinality = BatchCardinality,
> {
  readonly name: Name
  readonly from: From
  readonly graph: Child
  readonly to: To
  readonly parameter: KeyParameter['name']
  readonly cardinality: Cardinality
  readonly parameters: BatchStaticParameters<DefinitionOf<Child>, KeyParameter['name']>
  readonly ordering?: DefinitionOrderingName<DefinitionOf<Child>>
  readonly [batchRelationType]?: KeyParameter
}

export type BatchRelationConfiguration<
  Name extends string,
  Child extends RelationalQueryGraph,
  From extends string,
  To extends DefinitionProjectionPath<DefinitionOf<Child>>,
  ParameterReference extends ListParameterReference<DefinitionOf<Child>>,
  Cardinality extends BatchCardinality,
> = {
  name: Name
  from: From
  graph: Child
  to: To
  parameter: ParameterReference
  cardinality: Cardinality
  ordering?: DefinitionOrderingName<DefinitionOf<Child>>
} & BatchStaticParameterInput<DefinitionOf<Child>, ParameterReferenceName<NoInfer<ParameterReference>>>

export function batchRelation<
  const Name extends string,
  const Child extends RelationalQueryGraph,
  const From extends string,
  const To extends DefinitionProjectionPath<DefinitionOf<Child>>,
  const ParameterReference extends ListParameterReference<DefinitionOf<Child>>,
  const Cardinality extends BatchCardinality,
>(
  configuration: BatchRelationConfiguration<Name, Child, From, To, ParameterReference, Cardinality>,
): BatchRelation<Name, Child, From, To, ResolveListParameter<DefinitionOf<Child>, ParameterReference>, Cardinality>

type ProjectionScalarTypeAtPath<Definition extends GraphDefinitionInput, Path extends string> =
  ProjectionFieldByPath<DefinitionProjectionField<Definition>, Path> extends infer Field
    ? Field extends ProjectionTypeMetadata<infer _Path, infer Type, infer _Nullability, infer _SelectedByDefault>
      ? Type
      : Field extends ProjectionFieldDefinition<infer _Path, infer Type>
        ? Type
        : never
    : never

type EqualScalarType<Left, Right> = [Left] extends [Right] ? ([Right] extends [Left] ? true : false) : false

type CompatibleBatchRelation<Root extends RelationalQueryGraph, Relation extends BatchRelation> =
  Relation extends BatchRelation<infer Name, infer Child, infer From, infer To, infer KeyParameter, infer _Cardinality>
    ? From extends DefinitionProjectionPath<DefinitionOf<Root>>
      ? Extract<DefinitionProjectionPath<DefinitionOf<Root>>, Name | `${Name}.${string}`> extends never
        ? EqualScalarType<
            ProjectionScalarTypeAtPath<DefinitionOf<Root>, From>,
            ProjectionScalarTypeAtPath<DefinitionOf<Child>, To>
          > extends true
          ? EqualScalarType<
              ProjectionScalarTypeAtPath<DefinitionOf<Child>, To>,
              KeyParameter['scalarType']
            > extends true
            ? Relation
            : never
          : never
        : never
      : never
    : never

type CompatibleBatchRelations<Root extends RelationalQueryGraph, Relations extends readonly BatchRelation[]> = {
  readonly [Index in keyof Relations]: Relations[Index] extends BatchRelation
    ? CompatibleBatchRelation<Root, Relations[Index]>
    : never
}

type RelationName<Relations extends readonly BatchRelation[]> = Relations[number]['name']
type ChildPathsForRelation<Relation extends BatchRelation> = Relation extends BatchRelation
  ? `${Relation['name']}.${DefinitionProjectionPath<DefinitionOf<Relation['graph']>>}`
  : never
type ChildPaths<Relations extends readonly BatchRelation[]> = ChildPathsForRelation<Relations[number]>

export type ComposedSelection<Root extends RelationalQueryGraph, Relations extends readonly BatchRelation[]> =
  | DefinitionProjectionPath<DefinitionOf<Root>>
  | ChildPaths<Relations>

export interface BatchPlanMetadata<Relation extends BatchRelation = BatchRelation> {
  name: Relation['name']
  parentKey: Relation['from']
  childKey: Relation['to']
  keyParameter: Relation['parameter']
  parameters: Readonly<Relation['parameters']>
  cardinality: Relation['cardinality']
  parentKeyInjected: boolean
  childKeyInjected: boolean
}

type BatchRelationNamed<Relation extends BatchRelation, Name extends string> = Relation extends BatchRelation
  ? Relation['name'] extends Name
    ? Relation
    : never
  : never

type BatchRelationByName<Relations extends readonly BatchRelation[], Name extends string> = BatchRelationNamed<
  Relations[number],
  Name
>

type BatchKeyValue<Relation extends BatchRelation> =
  Relation extends BatchRelation<
    infer _Name,
    infer _Child,
    infer _From,
    infer _To,
    infer KeyParameter,
    infer _Cardinality
  >
    ? ScalarParameterValue<KeyParameter['scalarType']>
    : never

export interface CompiledQueryPlan<Relations extends readonly BatchRelation[] = readonly BatchRelation[]> {
  readonly root: import('./index.js').CompiledSqlStatement
  readonly batches: readonly BatchPlanMetadata<Relations[number]>[]
  compileBatch<const Name extends RelationName<Relations>>(
    name: Name,
    keys: readonly BatchKeyValue<BatchRelationByName<Relations, Name>>[],
  ): import('./index.js').CompiledSqlStatement
}

export type ComposedQueryOperation<
  Root extends RelationalQueryGraph,
  Relations extends readonly BatchRelation[],
> = QueryOperationBase<ComposedSelection<Root, Relations>, DefinitionOrderingName<DefinitionOf<Root>>> &
  OperationParameterInput<DefinitionOf<Root>>

export interface ComposedQueryGraph<
  Root extends RelationalQueryGraph = RelationalQueryGraph,
  Relations extends readonly BatchRelation[] = readonly BatchRelation[],
> {
  readonly name: string
  compileOraclePlan(
    operation: ComposedQueryOperation<Root, Relations>,
    options?: OracleCompileOptions,
  ): CompiledQueryPlan<Relations>
  compileSqlServerPlan(
    operation: ComposedQueryOperation<Root, Relations>,
    options?: SqlServerCompileOptions,
  ): CompiledQueryPlan<Relations>
}

export function composeGraph<
  const Root extends RelationalQueryGraph,
  const Relations extends readonly BatchRelation[],
>(configuration: {
  root: Root
  relations: Relations & CompatibleBatchRelations<Root, Relations>
}): ComposedQueryGraph<Root, Relations>

export interface ScalarOutputTypeMap {
  boolean: unknown
  int32: unknown
  int64: unknown
  float64: unknown
  decimal: unknown
  string: unknown
  date: unknown
  dateTime: unknown
  binary: unknown
  json: unknown
}

export interface DefaultScalarOutputTypeMap extends ScalarOutputTypeMap {
  boolean: boolean
  int32: number
  int64: number | string
  float64: number
  decimal: number | string
  string: string
  date: string
  dateTime: string
  binary: string
  json: JsonValue
}

export type DefinitionOf<Subject extends GraphDefinitionInput | QueryGraph | RelationalQueryGraph> =
  Subject extends GraphDefinitionInput
    ? Subject
    : Subject extends QueryGraph<infer Definition>
      ? Definition
      : Subject extends RelationalQueryGraph<infer Definition>
        ? Definition
        : never

type IncomingRelation<Definition extends GraphDefinitionInput, Source extends string> =
  DefinitionRelation<Definition> extends infer Candidate
    ? Candidate extends RelationDefinition<infer _Name, infer _From, infer To, infer _Required, infer _Cardinality>
      ? Source extends To
        ? Candidate
        : never
      : never
    : never

type RelationPathNullability<
  Definition extends GraphDefinitionInput,
  Relation,
  Visited extends string,
  Depth extends readonly unknown[],
> =
  Relation extends RelationDefinition<infer _Name, infer From, infer _To, infer Required, infer _Cardinality>
    ? BooleanOr<
        | (Required extends true ? false : true)
        | SourceOuterNullable<Definition, From, Visited, readonly [...Depth, unknown]>
      >
    : boolean

type SourceOuterNullable<
  Definition extends GraphDefinitionInput,
  Source extends string,
  Visited extends string = never,
  Depth extends readonly unknown[] = readonly [],
> = Depth['length'] extends 12
  ? boolean
  : string extends Source
    ? boolean
    : string extends DefinitionRoot<Definition>
      ? boolean
      : Source extends DefinitionRoot<Definition>
        ? false
        : Source extends Visited
          ? boolean
          : [IncomingRelation<Definition, Source>] extends [never]
            ? boolean
            : BooleanOr<
                RelationPathNullability<Definition, IncomingRelation<Definition, Source>, Visited | Source, Depth>
              >

type CombineAllNullability<Left extends boolean, Right extends boolean> = [Left] extends [false]
  ? false
  : [Right] extends [false]
    ? false
    : [Left] extends [true]
      ? Right
      : [Right] extends [true]
        ? Left
        : boolean

type EvaluateAnyNullability<
  Definition extends GraphDefinitionInput,
  Values,
  Depth extends readonly unknown[],
> = Values extends readonly []
  ? false
  : Values extends readonly [
        infer First extends NullabilityExpression,
        ...infer Rest extends readonly NullabilityExpression[],
      ]
    ? BooleanOr<
        | EvaluateNullability<Definition, First, readonly [...Depth, unknown]>
        | EvaluateAnyNullability<Definition, Rest, readonly [...Depth, unknown]>
      >
    : Values extends readonly (infer Value extends NullabilityExpression)[]
      ? BooleanOr<EvaluateNullability<Definition, Value, readonly [...Depth, unknown]>>
      : boolean

type EvaluateAllNullability<
  Definition extends GraphDefinitionInput,
  Values,
  Depth extends readonly unknown[],
> = Values extends readonly []
  ? true
  : Values extends readonly [
        infer First extends NullabilityExpression,
        ...infer Rest extends readonly NullabilityExpression[],
      ]
    ? CombineAllNullability<
        EvaluateNullability<Definition, First, readonly [...Depth, unknown]>,
        EvaluateAllNullability<Definition, Rest, readonly [...Depth, unknown]>
      >
    : boolean

type EvaluateNullability<
  Definition extends GraphDefinitionInput,
  Formula extends NullabilityExpression,
  Depth extends readonly unknown[] = readonly [],
> = Depth['length'] extends 16
  ? boolean
  : NullabilityExpression extends Formula
    ? boolean
    : Formula extends boolean
      ? Formula
      : Formula extends SourceFieldNullability<infer Source, infer FieldNullable>
        ? BooleanOr<FieldNullable | SourceOuterNullable<Definition, Source, never, Depth>>
        : Formula extends AnyNullability<infer Values>
          ? EvaluateAnyNullability<Definition, Values, Depth>
          : Formula extends AllNullability<infer Values>
            ? EvaluateAllNullability<Definition, Values, Depth>
            : boolean

type ProjectionFieldPath<Field> =
  Field extends ProjectionTypeMetadata<infer Path, infer _Type, infer _Nullability, infer _SelectedByDefault>
    ? Path
    : Field extends ProjectionFieldDefinition<infer Path>
      ? Path
      : never

type ProjectionFieldValue<Definition extends GraphDefinitionInput, Field, TypeMap extends ScalarOutputTypeMap> =
  Field extends ProjectionTypeMetadata<infer _Path, infer Type, infer Nullability, infer _SelectedByDefault>
    ? true extends EvaluateNullability<Definition, Nullability>
      ? TypeMap[Type] | null
      : TypeMap[Type]
    : Field extends ProjectionFieldDefinition<infer _Path, infer Type, infer Nullability, infer _SelectedByDefault>
      ? true extends EvaluateNullability<Definition, Nullability>
        ? TypeMap[Type] | null
        : TypeMap[Type]
      : never

type ProjectionFieldByPath<Field, Path extends string> = Field extends ProjectionFieldDefinition
  ? ProjectionFieldPath<Field> extends Path
    ? Field
    : never
  : never

type ProjectionDefaultAtPath<Field, Path extends string> =
  Field extends ProjectionTypeMetadata<infer FieldPath, infer _Type, infer _Nullability, infer SelectedByDefault>
    ? FieldPath extends Path
      ? SelectedByDefault
      : never
    : Field extends ProjectionFieldDefinition
      ? ProjectionFieldPath<Field> extends Path
        ? Field extends { selectedByDefault: true }
          ? true
          : false
        : never
      : never

type DefaultProjectionPath<Field, Path extends string = ProjectionFieldPath<Field>> = Path extends unknown
  ? [ProjectionDefaultAtPath<Field, Path>] extends [never]
    ? never
    : ProjectionDefaultAtPath<Field, Path> extends true
      ? Path
      : never
  : never

type DefaultProjectionField<Field> = ProjectionFieldByPath<Field, DefaultProjectionPath<Field>>

type SelectedProjectionField<Definition extends GraphDefinitionInput, Operation> = [Operation] extends [undefined]
  ? DefaultProjectionField<DefinitionProjectionField<Definition>>
  : Operation extends { select: readonly (infer Path extends string)[] }
    ? ProjectionFieldByPath<DefinitionProjectionField<Definition>, Path>
    : DefaultProjectionField<DefinitionProjectionField<Definition>>

type ProjectionEntry<
  Definition extends GraphDefinitionInput,
  Field,
  TypeMap extends ScalarOutputTypeMap,
> = Field extends ProjectionFieldDefinition
  ? {
      path: ProjectionFieldPath<Field>
      value: ProjectionFieldValue<Definition, Field, TypeMap>
    }
  : never

type ProjectionEntryHead<Entry> = Entry extends { path: infer Path extends string }
  ? Path extends `${infer Head}.${string}`
    ? Head
    : Path
  : never

type DirectProjectionEntryValue<Entry, Key extends string> = Entry extends {
  path: infer Path extends string
  value: infer Value
}
  ? Path extends Key
    ? Value
    : never
  : never

type NestedProjectionEntry<Entry, Key extends string> = Entry extends {
  path: infer Path extends string
  value: infer Value
}
  ? Path extends `${Key}.${infer Rest}`
    ? { path: Rest; value: Value }
    : never
  : never

type ProjectionValueAtKey<Entry, Key extends string> = [NestedProjectionEntry<Entry, Key>] extends [never]
  ? DirectProjectionEntryValue<Entry, Key>
  : [DirectProjectionEntryValue<Entry, Key>] extends [never]
    ? BuildProjectionResult<NestedProjectionEntry<Entry, Key>>
    : DirectProjectionEntryValue<Entry, Key> | BuildProjectionResult<NestedProjectionEntry<Entry, Key>>

type BuildProjectionResult<Entry> = [Entry] extends [never]
  ? Record<never, never>
  : {
      [Key in ProjectionEntryHead<Entry>]: ProjectionValueAtKey<Entry, Key>
    }

export type ResultOf<
  Subject extends GraphDefinitionInput | QueryGraph | RelationalQueryGraph | ComposedQueryGraph,
  Operation = undefined,
  TypeMap extends ScalarOutputTypeMap = DefaultScalarOutputTypeMap,
> =
  Subject extends ComposedQueryGraph<infer Root, infer Relations>
    ? ComposedResult<Root, Relations, Operation, TypeMap>
    : DefinitionOf<
          Extract<Subject, GraphDefinitionInput | QueryGraph | RelationalQueryGraph>
        > extends infer Definition extends GraphDefinitionInput
      ? BuildProjectionResult<ProjectionEntry<Definition, SelectedProjectionField<Definition, Operation>, TypeMap>>
      : never

type SelectedPaths<Operation> = Operation extends { select: readonly (infer Path extends string)[] } ? Path : never

type RootOperation<Operation, Relations extends readonly BatchRelation[]> = Operation extends {
  select: readonly string[]
}
  ? { select: readonly Exclude<SelectedPaths<Operation>, ChildPaths<Relations>>[] }
  : undefined

type SelectedChildPath<Operation, Name extends string> =
  Extract<SelectedPaths<Operation>, `${Name}.${string}`> extends `${Name}.${infer Path}` ? Path : never

type ChildOperation<Operation, Name extends string> = {
  select: readonly SelectedChildPath<Operation, Name>[]
}

type SelectedRelationCandidate<Relation extends BatchRelation, Operation> = Relation extends BatchRelation
  ? Extract<SelectedPaths<Operation>, `${Relation['name']}.${string}`> extends never
    ? never
    : Relation
  : never

type SelectedRelation<Relations extends readonly BatchRelation[], Operation> = SelectedRelationCandidate<
  Relations[number],
  Operation
>

type BatchResult<Relation extends BatchRelation, Operation, TypeMap extends ScalarOutputTypeMap> =
  ResultOf<Relation['graph'], ChildOperation<Operation, Relation['name']>, TypeMap> extends infer Child
    ? Relation['cardinality'] extends 'many'
      ? Child[]
      : Child | null
    : never

type ComposedResult<
  Root extends RelationalQueryGraph,
  Relations extends readonly BatchRelation[],
  Operation,
  TypeMap extends ScalarOutputTypeMap,
> = ResultOf<Root, RootOperation<Operation, Relations>, TypeMap> & {
  [Relation in SelectedRelation<Relations, Operation> as Relation['name']]: BatchResult<Relation, Operation, TypeMap>
}

export type SqlServerVersion = '2008' | '2012' | '2016' | '2019' | '2022'
export type OracleVersion = '11g' | '12c' | '19c' | '21c' | '23ai'

export interface SqlServerCompileOptions {
  version?: SqlServerVersion
}

export interface OracleCompileOptions {
  version?: OracleVersion
}

export type QueryGraphErrorPhase = 'definition' | 'mapping' | 'composition' | 'operation' | 'sql'

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

export interface FieldSpecDefinition<
  Type extends ScalarType = ScalarType,
  Nullable extends boolean = boolean,
  Selectable extends boolean = boolean,
> {
  scalarType: Type
  nullable?: Nullable
  selectable?: Selectable
}

export type FieldSpec = ScalarType | FieldSpecDefinition
export type FieldSpecMap = Record<string, FieldSpec>

export type FieldSpecScalarType<Specification extends FieldSpec> = Specification extends ScalarType
  ? Specification
  : Specification extends FieldSpecDefinition<infer Type, infer _Nullable, infer _Selectable>
    ? Type
    : never

export type FieldSpecIsNullable<Specification extends FieldSpec> = Specification extends ScalarType
  ? false
  : Specification extends FieldSpecDefinition<infer _Type, infer Nullable, infer _Selectable>
    ? Nullable
    : boolean

export type FieldSpecIsSelectable<Specification extends FieldSpec> = Specification extends ScalarType
  ? true
  : Specification extends FieldSpecDefinition<infer _Type, infer _Nullable, infer Selectable>
    ? Selectable
    : boolean

export interface SourceRef<Key extends string = string, Fields extends FieldSpecMap = FieldSpecMap>
  extends SourceDefinition {
  key: Key
  field<Name extends Extract<keyof Fields, string>>(
    name: Name,
  ): TypedExpression<
    FieldExpression<Key, Name>,
    FieldSpecScalarType<Fields[Name]>,
    SourceFieldNullability<Key, FieldSpecIsNullable<Fields[Name]>>
  >
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

export type RelationRef<
  Name extends string = string,
  From extends string = string,
  To extends string = string,
  Required extends boolean = boolean,
  Cardinality extends RelationCardinality = RelationCardinality,
> = RelationDefinition<Name, From, To, Required, Cardinality>

export type LiteralInput = null | boolean | string | number
export type ExpressionInput = Expression | LiteralInput | SummaryFieldDefinition

type NumberLiteralScalarType<Value extends number> = number extends Value
  ? 'int64' | 'decimal'
  : `${Value}` extends `${bigint}`
    ? 'int64'
    : 'decimal'

type InputScalarType<Input extends ExpressionInput> =
  Input extends ExpressionTypeMetadata<infer Type, infer _Nullability>
    ? Type
    : Input extends ProjectionFieldDefinition<infer _Path, infer Type, infer _Nullability, infer _SelectedByDefault>
      ? Type
      : Input extends null
        ? null
        : Input extends boolean
          ? 'boolean'
          : Input extends string
            ? 'string'
            : Input extends number
              ? NumberLiteralScalarType<Input>
              : ScalarType

type InputNullability<Input extends ExpressionInput> =
  Input extends ExpressionTypeMetadata<infer _Type, infer Nullability>
    ? Nullability
    : Input extends ProjectionFieldDefinition<infer _Path, infer _Type, infer Nullability, infer _SelectedByDefault>
      ? Nullability
      : Input extends null
        ? true
        : Input extends Expression
          ? boolean
          : false

export type ExpressionScalarType<Input extends ExpressionInput> = InputScalarType<Input>
export type ExpressionNullability<Input extends ExpressionInput> = InputNullability<Input>

type PromoteScalarType<Type extends ScalarType | null> = 'float64' extends Type
  ? 'float64'
  : 'decimal' extends Type
    ? 'decimal'
    : 'int64' extends Type
      ? 'int64'
      : 'int32' extends Type
        ? 'int32'
        : 'dateTime' extends Type
          ? 'dateTime'
          : Exclude<Type, null>

type AverageScalarType<Type extends ScalarType | null> = Type extends 'float64'
  ? 'float64'
  : Type extends 'int32' | 'int64' | 'decimal'
    ? 'decimal'
    : Exclude<Type, null>

type InputNullabilities<Inputs extends readonly ExpressionInput[]> = {
  [Index in keyof Inputs]: InputNullability<Inputs[Index]>
}

type AnyInputNullability<Inputs extends readonly ExpressionInput[]> = AnyNullability<InputNullabilities<Inputs>>
type AllInputNullability<Inputs extends readonly ExpressionInput[]> = AllNullability<InputNullabilities<Inputs>>

export interface FieldTypeOptions {
  nullable?: boolean
  selectable?: boolean
}

type OptionNullable<Options extends FieldTypeOptions> = Options extends { nullable: true } ? true : false
type OptionSelectable<Options extends FieldTypeOptions> = Options extends { selectable: false } ? false : true

export function fieldType<const Type extends ScalarType, const Options extends FieldTypeOptions = Record<never, never>>(
  scalarType: Type,
  options?: Options,
): FieldSpecDefinition<Type, OptionNullable<Options>, OptionSelectable<Options>>
export function nullable<const Specification extends FieldSpec>(
  specification: Specification,
): FieldSpecDefinition<FieldSpecScalarType<Specification>, true, FieldSpecIsSelectable<Specification>>
export function hidden<const Specification extends FieldSpec>(
  specification: Specification,
): FieldSpecDefinition<FieldSpecScalarType<Specification>, FieldSpecIsNullable<Specification>, false>

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
): TypedExpression<ParameterExpression<Name>, Type, false>

export function literal<const Value extends LiteralInput>(
  value: Value,
): TypedExpression<LiteralExpression, InputScalarType<Value>, InputNullability<Value>>
export function integer(value: number): TypedExpression<LiteralExpression, 'int64', false>
export function decimal(value: string | number): TypedExpression<LiteralExpression, 'decimal', false>

type EqualityNullability<Left extends ExpressionInput, Right extends ExpressionInput> = null extends
  | InputScalarType<Left>
  | InputScalarType<Right>
  ? false
  : AnyInputNullability<readonly [Left, Right]>

export function eq<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', EqualityNullability<Left, Right>>
export function neq<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', EqualityNullability<Left, Right>>
export function lt<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', AnyInputNullability<readonly [Left, Right]>>
export function lte<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', AnyInputNullability<readonly [Left, Right]>>
export function gt<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', AnyInputNullability<readonly [Left, Right]>>
export function gte<const Left extends ExpressionInput, const Right extends ExpressionInput>(
  left: Left,
  right: Right,
): TypedExpression<BinaryExpression, 'boolean', AnyInputNullability<readonly [Left, Right]>>
export function like<const Value extends ExpressionInput, const Pattern extends ExpressionInput>(
  expression: Value,
  pattern: Pattern,
): TypedExpression<LikeExpression, 'boolean', AnyInputNullability<readonly [Value, Pattern]>>
export function inList<const Value extends ExpressionInput, const Values extends readonly ExpressionInput[]>(
  expression: Value,
  values: Values,
): TypedExpression<InExpression, 'boolean', AnyInputNullability<readonly [Value, ...Values]>>
export function and<const Expressions extends readonly ExpressionInput[]>(
  ...expressions: Expressions
): TypedExpression<ExpressionGroup, 'boolean', AnyInputNullability<Expressions>>
export function inParameter<
  const Value extends ExpressionInput,
  const Name extends string,
  const Type extends ScalarType,
>(
  expression: Value,
  parameter: ListParameterRef<Name, Type>,
): TypedExpression<InParameterExpression<Name>, 'boolean', InputNullability<Value>>
export function or<const Expressions extends readonly ExpressionInput[]>(
  ...expressions: Expressions
): TypedExpression<ExpressionGroup, 'boolean', AnyInputNullability<Expressions>>
export function not<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<UnaryExpression, 'boolean', InputNullability<Value>>
export function isNull(expression: ExpressionInput): TypedExpression<UnaryExpression, 'boolean', false>
export function isNotNull(expression: ExpressionInput): TypedExpression<UnaryExpression, 'boolean', false>
export function exists<const Source extends string>(
  source: Source | SourceRef<Source>,
  predicate?: ExpressionInput,
): TypedExpression<ExistsExpression<Source, never>, 'boolean', false>
export interface ExistsConfiguration<From extends string = string> {
  from: From | SourceRef<From>
}
export function exists<const Source extends string, const From extends string>(
  source: Source | SourceRef<Source>,
  predicate: ExpressionInput | undefined,
  configuration: ExistsConfiguration<From>,
): TypedExpression<ExistsExpression<Source, From>, 'boolean', false>

export function lower<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<FunctionExpression, 'string', InputNullability<Value>>
export function upper<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<FunctionExpression, 'string', InputNullability<Value>>
export function coalesce<
  const First extends ExpressionInput,
  const Second extends ExpressionInput,
  const Rest extends readonly ExpressionInput[],
>(
  first: First,
  second: Second,
  ...rest: Rest
): TypedExpression<
  FunctionExpression,
  PromoteScalarType<InputScalarType<First | Second | Rest[number]>>,
  AllInputNullability<readonly [First, Second, ...Rest]>
>
export function concat<const First extends ExpressionInput, const Rest extends readonly ExpressionInput[]>(
  first: First,
  ...rest: Rest
): TypedExpression<FunctionExpression, 'string', AllInputNullability<readonly [First, ...Rest]>>

export function count(expression?: ExpressionInput): TypedExpression<AggregateExpression, 'int64', false>
export function countDistinct(expression: ExpressionInput): TypedExpression<AggregateExpression, 'int64', false>
export function sum<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<AggregateExpression, Exclude<InputScalarType<Value>, null>, true>
export function average<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<AggregateExpression, AverageScalarType<InputScalarType<Value>>, true>
export function minimum<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<AggregateExpression, Exclude<InputScalarType<Value>, null>, true>
export function maximum<const Value extends ExpressionInput>(
  expression: Value,
): TypedExpression<AggregateExpression, Exclude<InputScalarType<Value>, null>, true>

export type SourceReferenceKey<Reference extends string | SourceRef> =
  Reference extends SourceRef<infer Key, infer _Fields> ? Key : Reference extends string ? Reference : string

export interface RelationConfiguration<
  Name extends string = string,
  From extends string | SourceRef = string | SourceRef,
  To extends string | SourceRef = string | SourceRef,
  Required extends boolean = boolean,
  Cardinality extends RelationCardinality = RelationCardinality,
> {
  name: Name
  from: From
  to: To
  on: Expression
  required?: Required
  cardinality?: Cardinality
  selection?: RelationSelection
}

export function relation<
  const Name extends string,
  const From extends string | SourceRef,
  const To extends string | SourceRef,
  const Required extends boolean = false,
  const Cardinality extends RelationCardinality = 'one',
>(
  configuration: RelationConfiguration<Name, From, To, Required, Cardinality>,
): RelationRef<Name, SourceReferenceKey<From>, SourceReferenceKey<To>, Required, Cardinality>

export interface ConstraintConfiguration {
  predicate: Expression
  when?: string | ParameterRef
}

export function constraint(configuration: ConstraintConfiguration): ConstraintDefinition

export interface ProjectionConfiguration<
  Path extends string | readonly string[] = string | readonly string[],
  Value extends ExpressionInput = ExpressionInput,
  SelectedByDefault extends boolean = boolean,
> {
  path: Path
  expression: Value
  default?: SelectedByDefault
}

type ConfigurationPath<Path extends string | readonly string[]> = Path extends string
  ? Path
  : Path extends readonly string[]
    ? JoinProjectionPath<Path>
    : never

export function project<
  const Path extends string | readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  configuration: ProjectionConfiguration<Path, Value, SelectedByDefault>,
): TypedProjectionField<
  ConfigurationPath<Path>,
  Extract<InputScalarType<Value>, ScalarType>,
  InputNullability<Value>,
  SelectedByDefault
>

export function dimension<
  const Path extends string | readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  configuration: ProjectionConfiguration<Path, Value, SelectedByDefault>,
): TypedDimensionDefinition<
  ConfigurationPath<Path>,
  Extract<InputScalarType<Value>, ScalarType>,
  InputNullability<Value>,
  SelectedByDefault
>

export function measure<
  const Path extends string | readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  configuration: ProjectionConfiguration<Path, Value, SelectedByDefault>,
): TypedMeasureDefinition<
  ConfigurationPath<Path>,
  Extract<InputScalarType<Value>, ScalarType>,
  InputNullability<Value>,
  SelectedByDefault
>

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
  Module extends GraphModule<
    infer Parameter,
    infer _ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer _Relation
  >
    ? Parameter
    : never

type ModuleProjectionPath<Module> =
  Module extends GraphModule<
    infer _Parameter,
    infer ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer _Relation
  >
    ? ProjectionPath
    : never

type ModuleOrderingName<Module> =
  Module extends GraphModule<
    infer _Parameter,
    infer _ProjectionPath,
    infer OrderingName,
    infer _ProjectionField,
    infer _Relation
  >
    ? OrderingName
    : never

type ModuleProjectionField<Module> =
  Module extends GraphModule<
    infer _Parameter,
    infer _ProjectionPath,
    infer _OrderingName,
    infer ProjectionField,
    infer _Relation
  >
    ? ProjectionField
    : never

type ModuleRelation<Module> =
  Module extends GraphModule<
    infer _Parameter,
    infer _ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer Relation
  >
    ? Relation
    : never

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

type ConfigurationProjectionField<Configuration> =
  | Extract<ConfigurationElement<Configuration, 'projection'>, ProjectionFieldDefinition>
  | Extract<ConfigurationElement<Configuration, 'dimensions'>, ProjectionFieldDefinition>
  | Extract<ConfigurationElement<Configuration, 'measures'>, ProjectionFieldDefinition>
  | ModuleProjectionField<ConfigurationElement<Configuration, 'modules'>>

type ConfigurationOrderingName<Configuration> =
  | OrderingNameOf<ConfigurationElement<Configuration, 'orderings'>>
  | ModuleOrderingName<ConfigurationElement<Configuration, 'modules'>>

type ConfigurationRelation<Configuration> =
  | Extract<ConfigurationElement<Configuration, 'relations'>, RelationDefinition>
  | ModuleRelation<ConfigurationElement<Configuration, 'modules'>>

type ConfigurationRoot<Configuration> = Configuration extends {
  root: infer Root extends string | SourceRef
}
  ? SourceReferenceKey<Root>
  : string

export interface GraphModule<
  Parameter extends ParameterDefinition = ParameterDefinition,
  ProjectionPath extends string = string,
  OrderingName extends string = string,
  ProjectionField extends ProjectionFieldDefinition = ProjectionFieldDefinition<ProjectionPath>,
  Relation extends RelationDefinition = RelationDefinition,
> {
  readonly name: string
  readonly sources: readonly SourceRef[]
  readonly parameters: readonly Parameter[]
  readonly relations: readonly Relation[]
  readonly constraints: readonly ConstraintDefinition[]
  readonly projection: readonly ProjectionField[]
  readonly orderings: readonly OrderingDefinition<OrderingName>[]
}

export function defineGraphModule<const Configuration extends GraphModuleConfiguration>(
  configuration: Configuration,
): GraphModule<
  ConfigurationParameter<Configuration>,
  ConfigurationProjectionPath<Configuration>,
  ConfigurationOrderingName<Configuration>,
  ConfigurationProjectionField<Configuration>,
  ConfigurationRelation<Configuration>
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
  ConfigurationOrderingName<Configuration>,
  ConfigurationProjectionField<Configuration>,
  ConfigurationRelation<Configuration>,
  ConfigurationRoot<Configuration>
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
  ConfigurationOrderingName<Configuration>,
  ConfigurationProjectionField<Configuration>,
  ConfigurationRelation<Configuration>,
  ConfigurationRoot<Configuration>
>
