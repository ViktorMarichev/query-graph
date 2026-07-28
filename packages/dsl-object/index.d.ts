export * from 'query-graph/definition'

import type {
  ConstraintDefinition,
  DimensionDefinition,
  Expression,
  ExpressionInput,
  JoinProjectionPath,
  MeasureDefinition,
  ParameterRef,
  ProjectionFieldDefinition,
  RelationCardinality,
  RelationRef,
  RelationSelection,
  SourceRef,
} from 'query-graph/definition'

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
  name: string
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
