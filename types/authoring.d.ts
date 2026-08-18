import type {
  AggregateExpression,
  AllNullability,
  AnyNullability,
  BinaryExpression,
  ConstraintDefinition,
  DimensionDefinition,
  ExistsExpression,
  Expression,
  ExpressionGroup,
  ExpressionTypeMetadata,
  FieldExpression,
  FirstBySelection,
  FunctionExpression,
  GraphDefinition,
  InExpression,
  InParameterExpression,
  JoinProjectionPath,
  LikeExpression,
  LiteralExpression,
  MeasureDefinition,
  NullsOrder,
  OrderByDefinition,
  OrderingDefinition,
  ParameterDefinition,
  ParameterExpression,
  ProjectionFieldDefinition,
  ProjectionObjectDefinition,
  RelationCardinality,
  RelationDefinition,
  RelationSelection,
  ScalarType,
  SourceDefinition,
  SourceFieldNullability,
  SummaryFieldDefinition,
  TypedDimensionDefinition,
  TypedExpression,
  TypedMeasureDefinition,
  TypedProjectionField,
  TypedProjectionObject,
  UnaryExpression,
} from './model.js'

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

export interface ProjectionObjectConfiguration<
  Path extends string | readonly string[] = string | readonly string[],
  Value extends ExpressionInput = ExpressionInput,
> {
  path: Path
  presence: Value
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

export function projectObject<const Path extends string | readonly string[], const Value extends ExpressionInput>(
  configuration: ProjectionObjectConfiguration<Path, Value>,
): TypedProjectionObject<ConfigurationPath<Path>, InputNullability<Value>>

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
  objects?: readonly ProjectionObjectDefinition[]
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

type ModuleProjectionObject<Module> =
  Module extends GraphModule<
    infer _Parameter,
    infer _ProjectionPath,
    infer _OrderingName,
    infer _ProjectionField,
    infer _Relation,
    infer ProjectionObject
  >
    ? ProjectionObject
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

type ConfigurationProjectionObject<Configuration> =
  | Extract<ConfigurationElement<Configuration, 'objects'>, ProjectionObjectDefinition>
  | ModuleProjectionObject<ConfigurationElement<Configuration, 'modules'>>

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
  ProjectionObject extends ProjectionObjectDefinition = ProjectionObjectDefinition,
> {
  readonly name: string
  readonly sources: readonly SourceRef[]
  readonly parameters: readonly Parameter[]
  readonly relations: readonly Relation[]
  readonly constraints: readonly ConstraintDefinition[]
  readonly projection: readonly ProjectionField[]
  readonly objects: readonly ProjectionObject[]
  readonly orderings: readonly OrderingDefinition<OrderingName>[]
}

export function defineGraphModule<const Configuration extends GraphModuleConfiguration>(
  configuration: Configuration,
): GraphModule<
  ConfigurationParameter<Configuration>,
  ConfigurationProjectionPath<Configuration>,
  ConfigurationOrderingName<Configuration>,
  ConfigurationProjectionField<Configuration>,
  ConfigurationRelation<Configuration>,
  ConfigurationProjectionObject<Configuration>
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
  objects?: readonly ProjectionObjectDefinition[]
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
  ConfigurationRoot<Configuration>,
  ConfigurationProjectionObject<Configuration>
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
  ConfigurationRoot<Configuration>,
  never
>
