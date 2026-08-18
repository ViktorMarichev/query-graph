import type {
  GraphDefinitionInput,
  ParameterDefinition,
  ProjectionFieldDefinition,
  ProjectionTypeMetadata,
  RelationCardinality,
  ScalarParameterValue,
} from './model.js'
import type {
  DefinitionOf,
  DefinitionOrderingName,
  DefinitionParameter,
  DefinitionProjectionField,
  DefinitionProjectionPath,
  OperationParameters,
  OracleCompileOptions,
  ParameterValue,
  QueryOperationBase,
  RelationalQueryGraph,
  SqlServerCompileOptions,
} from './runtime.js'
import type { ListParameterRef } from './authoring.js'

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

type OperationParameterInput<Definition extends GraphDefinitionInput> = [DefinitionParameter<Definition>] extends [
  never,
]
  ? { parameters?: never }
  : [RequiredParameter<DefinitionParameter<Definition>>] extends [never]
    ? { parameters?: OperationParameters<Definition> }
    : { parameters: OperationParameters<Definition> }

type ProjectionFieldPath<Field> =
  Field extends ProjectionTypeMetadata<infer Path, infer _Type, infer _Nullability, infer _SelectedByDefault>
    ? Path
    : Field extends ProjectionFieldDefinition<infer Path>
      ? Path
      : never

type ProjectionFieldByPath<Field, Path extends string> = Field extends ProjectionFieldDefinition
  ? ProjectionFieldPath<Field> extends Path
    ? Field
    : never
  : never

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

declare const batchQueryType: unique symbol

export interface BatchQuery<
  Child extends RelationalQueryGraph = RelationalQueryGraph,
  KeyPath extends string = string,
  KeyParameter extends ParameterDefinition & { shape: 'list' } = ListParameterRef,
> {
  readonly graph: Child
  readonly key: {
    readonly path: KeyPath
    readonly parameter: KeyParameter['name']
  }
  readonly [batchQueryType]?: KeyParameter
}

export type BatchQueryConfiguration<
  Child extends RelationalQueryGraph,
  KeyPath extends DefinitionProjectionPath<DefinitionOf<Child>>,
  ParameterReference extends ListParameterReference<DefinitionOf<Child>>,
> = {
  graph: Child
  key: {
    path: KeyPath
    parameter: ParameterReference
  }
}

type CompatibleBatchQueryConfiguration<
  Child extends RelationalQueryGraph,
  KeyPath extends DefinitionProjectionPath<DefinitionOf<Child>>,
  KeyParameter extends ParameterDefinition & { shape: 'list' },
> =
  EqualScalarType<ProjectionScalarTypeAtPath<DefinitionOf<Child>, KeyPath>, KeyParameter['scalarType']> extends true
    ? unknown
    : never

export function batchQuery<
  const Child extends RelationalQueryGraph,
  const KeyPath extends DefinitionProjectionPath<DefinitionOf<Child>>,
  const ParameterReference extends ListParameterReference<DefinitionOf<Child>>,
>(
  configuration: BatchQueryConfiguration<Child, KeyPath, ParameterReference> &
    CompatibleBatchQueryConfiguration<
      Child,
      KeyPath,
      ResolveListParameter<DefinitionOf<Child>, NoInfer<ParameterReference>>
    >,
): BatchQuery<Child, KeyPath, ResolveListParameter<DefinitionOf<Child>, ParameterReference>>

type BatchQueryGraph<Query extends BatchQuery> =
  Query extends BatchQuery<infer Child, infer _KeyPath, infer _KeyParameter> ? Child : never

type BatchQueryKeyPath<Query extends BatchQuery> =
  Query extends BatchQuery<infer _Child, infer KeyPath, infer _KeyParameter> ? KeyPath : never

type BatchQueryKeyParameter<Query extends BatchQuery> =
  Query extends BatchQuery<infer _Child, infer _KeyPath, infer KeyParameter> ? KeyParameter : never

declare const batchRelationType: unique symbol

export interface BatchRelation<
  Name extends string = string,
  Query extends BatchQuery = BatchQuery,
  From extends string = string,
  Cardinality extends BatchCardinality = BatchCardinality,
> {
  readonly name: Name
  readonly from: From
  readonly query: Query
  readonly cardinality: Cardinality
  readonly parameters: BatchStaticParameters<
    DefinitionOf<BatchQueryGraph<Query>>,
    BatchQueryKeyParameter<Query>['name']
  >
  readonly ordering?: DefinitionOrderingName<DefinitionOf<BatchQueryGraph<Query>>>
  readonly [batchRelationType]?: Query
}

export type BatchRelationConfiguration<
  Name extends string,
  Query extends BatchQuery,
  From extends string,
  Cardinality extends BatchCardinality,
> = {
  name: Name
  from: From
  query: Query
  cardinality: Cardinality
  ordering?: DefinitionOrderingName<DefinitionOf<BatchQueryGraph<Query>>>
} & BatchStaticParameterInput<DefinitionOf<BatchQueryGraph<Query>>, BatchQueryKeyParameter<Query>['name']>

export function batchRelation<
  const Name extends string,
  const Query extends BatchQuery,
  const From extends string,
  const Cardinality extends BatchCardinality,
>(
  configuration: BatchRelationConfiguration<Name, Query, From, Cardinality>,
): BatchRelation<Name, Query, From, Cardinality>

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
  Relation extends BatchRelation<infer Name, infer Query, infer From, infer _Cardinality>
    ? From extends DefinitionProjectionPath<DefinitionOf<Root>>
      ? Extract<DefinitionProjectionPath<DefinitionOf<Root>>, Name | `${Name}.${string}`> extends never
        ? EqualScalarType<
            ProjectionScalarTypeAtPath<DefinitionOf<Root>, From>,
            ProjectionScalarTypeAtPath<DefinitionOf<BatchQueryGraph<Query>>, BatchQueryKeyPath<Query>>
          > extends true
          ? EqualScalarType<
              ProjectionScalarTypeAtPath<DefinitionOf<BatchQueryGraph<Query>>, BatchQueryKeyPath<Query>>,
              BatchQueryKeyParameter<Query>['scalarType']
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
  ? `${Relation['name']}.${DefinitionProjectionPath<DefinitionOf<BatchQueryGraph<Relation['query']>>>}`
  : never
type ChildPaths<Relations extends readonly BatchRelation[]> = ChildPathsForRelation<Relations[number]>

export type ComposedSelection<Root extends RelationalQueryGraph, Relations extends readonly BatchRelation[]> =
  | DefinitionProjectionPath<DefinitionOf<Root>>
  | ChildPaths<Relations>

export interface BatchPlanMetadata<Relation extends BatchRelation = BatchRelation> {
  name: Relation['name']
  parentKey: Relation['from']
  childKey: BatchQueryKeyPath<Relation['query']>
  keyParameter: BatchQueryKeyParameter<Relation['query']>['name']
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

type BatchKeyValue<Relation extends BatchRelation> = ScalarParameterValue<
  BatchQueryKeyParameter<Relation['query']>['scalarType']
>

export interface CompiledQueryPlan<Relations extends readonly BatchRelation[] = readonly BatchRelation[]> {
  readonly root: import('../native.js').CompiledSqlStatement
  readonly batches: readonly BatchPlanMetadata<Relations[number]>[]
  compileBatch<const Name extends RelationName<Relations>>(
    name: Name,
    keys: readonly BatchKeyValue<BatchRelationByName<Relations, Name>>[],
  ): import('../native.js').CompiledSqlStatement
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
