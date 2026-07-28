export {
  and,
  asc,
  average,
  coalesce,
  concat,
  count,
  countDistinct,
  decimal,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  desc,
  eq,
  exists,
  fieldType,
  firstBy,
  gt,
  gte,
  hidden,
  inList,
  inParameter,
  integer,
  isNotNull,
  isNull,
  like,
  literal,
  lower,
  lt,
  lte,
  maximum,
  minimum,
  neq,
  not,
  nullable,
  optionalListParameter,
  optionalParameter,
  or,
  param,
  requiredListParameter,
  requiredParameter,
  source,
  sum,
  upper,
} from 'query-graph/definition'

export type {
  AggregateExpression,
  AggregateFunctionName,
  ConstraintDefinition,
  DimensionDefinition,
  Expression,
  ExpressionInput,
  FieldExpression,
  FieldSpec,
  FieldSpecDefinition,
  FieldSpecMap,
  FieldTypeOptions,
  FirstBySelection,
  GraphConfiguration,
  GraphDefinition,
  GraphModule,
  GraphModuleConfiguration,
  JoinProjectionPath,
  ListParameterRef,
  MeasureDefinition,
  NullsOrder,
  OrderByDefinition,
  OrderByOptions,
  ParameterDefinition,
  ParameterRef,
  ProjectionFieldDefinition,
  RelationCardinality,
  RelationDefinition,
  RelationRef,
  RelationSelection,
  ScalarParameterRef,
  ScalarParameterValue,
  ScalarType,
  SourceDefinition,
  SourceRef,
  SummaryFieldDefinition,
  SummaryGraphConfiguration,
} from 'query-graph/definition'

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
  from: SourceRef
  to: SourceRef
  on: Expression
  required?: boolean
  cardinality?: RelationCardinality
  selection?: RelationSelection
}

export function relation<const Name extends string>(configuration: RelationConfiguration<Name>): RelationRef<Name>

export interface ConstraintConfiguration {
  name: string
  predicate: Expression
  when?: ParameterRef
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
