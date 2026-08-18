/**
 * @deprecated Import the object authoring API from `@query-graph/core/dsl`.
 * This compatibility entrypoint will be removed in 2.0.
 */
export * from './dsl.js'

import type {
  ConstraintDefinition,
  Expression,
  ExpressionInput,
  ExpressionNullability,
  ExpressionScalarType,
  FieldExpression,
  FieldSpecIsNullable,
  FieldSpecMap,
  FieldSpecScalarType,
  JoinProjectionPath,
  ParameterRef,
  RelationCardinality,
  RelationRef,
  RelationSelection,
  ScalarType,
  SourceFieldNullability,
  SourceReferenceKey,
  SourceRef,
  TypedExpression,
  TypedDimensionDefinition,
  TypedMeasureDefinition,
  TypedProjectionField,
} from './dsl.js'

/** @deprecated Import authoring primitives from `@query-graph/core/dsl`. */
export const GRAPH_DEFINITION_VERSION: 10

/** @deprecated Use `source.field(name)` from `@query-graph/core/dsl`. */
export function field<
  const Key extends string,
  const Fields extends FieldSpecMap,
  const Name extends Extract<keyof Fields, string>,
>(
  source: SourceRef<Key, Fields>,
  name: Name,
): TypedExpression<
  FieldExpression<Key, Name>,
  FieldSpecScalarType<Fields[Name]>,
  SourceFieldNullability<Key, FieldSpecIsNullable<Fields[Name]>>
>
export function field<const Source extends string, const Name extends string>(
  source: Source,
  name: Name,
): TypedExpression<FieldExpression<Source, Name>, ScalarType, SourceFieldNullability<Source, boolean>>

export interface RelationOptions<
  Required extends boolean = boolean,
  Cardinality extends RelationCardinality = RelationCardinality,
> {
  required?: Required
  cardinality?: Cardinality
  selection?: RelationSelection
}

/** @deprecated Use `relation({ name, from, to, on, ...options })`. */
export function relation<
  const Name extends string,
  const From extends string | SourceRef,
  const To extends string | SourceRef,
  const Required extends boolean = false,
  const Cardinality extends RelationCardinality = 'one',
>(
  name: Name,
  from: From,
  to: To,
  on: Expression,
  options?: RelationOptions<Required, Cardinality>,
): RelationRef<Name, SourceReferenceKey<From>, SourceReferenceKey<To>, Required, Cardinality>

export interface ConstraintOptions {
  when?: string | ParameterRef
}

/** @deprecated Use `constraint({ predicate, when })`. */
export function constraint(predicate: Expression, options?: ConstraintOptions): ConstraintDefinition

export interface ProjectionOptions<SelectedByDefault extends boolean = boolean> {
  default?: SelectedByDefault
}

/** @deprecated Use `project({ path, expression, default })`. */
export function project<
  const Path extends string,
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedProjectionField<
  Path,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>
export function project<
  const Path extends readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedProjectionField<
  JoinProjectionPath<Path>,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>

/** @deprecated Use `dimension({ path, expression, default })`. */
export function dimension<
  const Path extends string,
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedDimensionDefinition<
  Path,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>
export function dimension<
  const Path extends readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedDimensionDefinition<
  JoinProjectionPath<Path>,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>

/** @deprecated Use `measure({ path, expression, default })`. */
export function measure<
  const Path extends string,
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedMeasureDefinition<
  Path,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>
export function measure<
  const Path extends readonly string[],
  const Value extends ExpressionInput,
  const SelectedByDefault extends boolean = false,
>(
  path: Path,
  expression: Value,
  options?: ProjectionOptions<SelectedByDefault>,
): TypedMeasureDefinition<
  JoinProjectionPath<Path>,
  Extract<ExpressionScalarType<Value>, ScalarType>,
  ExpressionNullability<Value>,
  SelectedByDefault
>
