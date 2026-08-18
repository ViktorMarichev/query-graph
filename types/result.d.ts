import type {
  AllNullability,
  AnyNullability,
  GraphDefinitionInput,
  JsonValue,
  NullabilityExpression,
  ProjectionFieldDefinition,
  ProjectionObjectDefinition,
  ProjectionObjectTypeMetadata,
  ProjectionTypeMetadata,
  RelationDefinition,
  SourceFieldNullability,
} from './model.js'
import type {
  DefinitionOf,
  DefinitionProjectionField,
  DefinitionProjectionObject,
  DefinitionProjectionPath,
  DefinitionRelation,
  DefinitionRoot,
  QueryGraph,
  RelationalQueryGraph,
} from './runtime.js'
import type { BatchQuery, BatchRelation, ComposedQueryGraph } from './composition.js'

type BooleanOr<Value extends boolean> = true extends Value ? true : false

type BatchQueryGraph<Query extends BatchQuery> =
  Query extends BatchQuery<infer Child, infer _KeyPath, infer _KeyParameter> ? Child : never

type ChildPathsForRelation<Relation extends BatchRelation> = Relation extends BatchRelation
  ? `${Relation['name']}.${DefinitionProjectionPath<DefinitionOf<BatchQueryGraph<Relation['query']>>>}`
  : never

type ChildPaths<Relations extends readonly BatchRelation[]> = ChildPathsForRelation<Relations[number]>

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

type ProjectionObjectPath<Object> =
  Object extends ProjectionObjectTypeMetadata<infer Path, infer _Nullability>
    ? Path
    : Object extends ProjectionObjectDefinition<infer Path>
      ? Path
      : never

type ProjectionObjectNullability<Object> =
  Object extends ProjectionObjectTypeMetadata<infer _Path, infer Nullability>
    ? Nullability
    : Object extends ProjectionObjectDefinition<infer _Path, infer Nullability>
      ? Nullability
      : boolean

type ProjectionObjectIsNullable<Definition extends GraphDefinitionInput, Object> =
  true extends EvaluateNullability<Definition, ProjectionObjectNullability<Object>> ? true : false

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

type ProjectionObjectForField<Object, Field> = Object extends ProjectionObjectDefinition
  ? Field extends ProjectionFieldDefinition
    ? ProjectionFieldPath<Field> extends `${ProjectionObjectPath<Object>}.${string}`
      ? Object
      : never
    : never
  : never

type SelectedProjectionObject<Definition extends GraphDefinitionInput, Field> = ProjectionObjectForField<
  DefinitionProjectionObject<Definition>,
  Field
>

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

type ProjectionObjectEntry<Definition extends GraphDefinitionInput, Object> = Object extends ProjectionObjectDefinition
  ? {
      path: ProjectionObjectPath<Object>
      nullable: ProjectionObjectIsNullable<Definition, Object>
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

type DirectProjectionObjectNullable<ObjectEntry, Key extends string> = ObjectEntry extends {
  path: infer Path extends string
  nullable: infer Nullable extends boolean
}
  ? Path extends Key
    ? Nullable
    : never
  : never

type NestedProjectionObjectEntry<ObjectEntry, Key extends string> = ObjectEntry extends {
  path: infer Path extends string
  nullable: infer Nullable extends boolean
}
  ? Path extends `${Key}.${infer Rest}`
    ? { path: Rest; nullable: Nullable }
    : never
  : never

type ApplyProjectionObjectNullability<Value, ObjectEntry, Key extends string> =
  true extends DirectProjectionObjectNullable<ObjectEntry, Key> ? Value | null : Value

type ProjectionValueAtKeyWithoutObject<Entry, ObjectEntry, Key extends string> = [
  NestedProjectionEntry<Entry, Key>,
] extends [never]
  ? DirectProjectionEntryValue<Entry, Key>
  : [DirectProjectionEntryValue<Entry, Key>] extends [never]
    ? BuildProjectionResult<NestedProjectionEntry<Entry, Key>, NestedProjectionObjectEntry<ObjectEntry, Key>>
    :
        | DirectProjectionEntryValue<Entry, Key>
        | BuildProjectionResult<NestedProjectionEntry<Entry, Key>, NestedProjectionObjectEntry<ObjectEntry, Key>>

type ProjectionValueAtKey<Entry, ObjectEntry, Key extends string> = ApplyProjectionObjectNullability<
  ProjectionValueAtKeyWithoutObject<Entry, ObjectEntry, Key>,
  ObjectEntry,
  Key
>

type BuildProjectionResult<Entry, ObjectEntry = never> = [Entry] extends [never]
  ? Record<never, never>
  : {
      [Key in ProjectionEntryHead<Entry>]: ProjectionValueAtKey<Entry, ObjectEntry, Key>
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
      ? SelectedProjectionField<Definition, Operation> extends infer Field
        ? BuildProjectionResult<
            ProjectionEntry<Definition, Field, TypeMap>,
            ProjectionObjectEntry<Definition, SelectedProjectionObject<Definition, Field>>
          >
        : never
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
  ResultOf<BatchQueryGraph<Relation['query']>, ChildOperation<Operation, Relation['name']>, TypeMap> extends infer Child
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
