export * from './dsl.js'

import type {
  ConstraintDefinition,
  DimensionDefinition,
  Expression,
  ExpressionInput,
  FieldExpression,
  FieldSpecMap,
  JoinProjectionPath,
  MeasureDefinition,
  ParameterRef,
  ProjectionFieldDefinition,
  RelationCardinality,
  RelationRef,
  RelationSelection,
  SourceRef,
} from './dsl.js'

export const GRAPH_DEFINITION_VERSION: 7

export function field<
  const Key extends string,
  const Fields extends FieldSpecMap,
  const Name extends Extract<keyof Fields, string>,
>(source: SourceRef<Key, Fields>, name: Name): FieldExpression<Key, Name>
export function field<const Source extends string, const Name extends string>(
  source: Source,
  name: Name,
): FieldExpression<Source, Name>

export interface RelationOptions {
  required?: boolean
  cardinality?: RelationCardinality
  selection?: RelationSelection
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

export function project<const Path extends string>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): ProjectionFieldDefinition<Path>
export function project<const Path extends readonly string[]>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): ProjectionFieldDefinition<JoinProjectionPath<Path>>

export function dimension<const Path extends string>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): DimensionDefinition<Path>
export function dimension<const Path extends readonly string[]>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): DimensionDefinition<JoinProjectionPath<Path>>

export function measure<const Path extends string>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): MeasureDefinition<Path>
export function measure<const Path extends readonly string[]>(
  path: Path,
  expression: ExpressionInput,
  options?: ProjectionOptions,
): MeasureDefinition<JoinProjectionPath<Path>>
