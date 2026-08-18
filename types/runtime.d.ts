import type {
  GraphDefinition,
  GraphDefinitionInput,
  OrderingDefinition,
  ParameterDefinition,
  ProjectionDefinitionInput,
  RelationalMapping,
  ScalarParameterValue,
} from './model.js'

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
export type DefinitionProjectionObject<Definition extends GraphDefinitionInput> = NonNullable<
  NonNullable<Definition['projection']>['objects']
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
  withRelationalMappings(mappings: readonly RelationalMapping[]): RelationalQueryGraph<Definition>
}

export interface RelationalQueryGraph<Definition extends GraphDefinitionInput = GraphDefinitionInput> {
  readonly [graphDefinitionType]?: Definition
  readonly name: string
  compileSqlServer(
    operation: QueryOperation<Definition>,
    options?: SqlServerCompileOptions,
  ): import('../native.js').CompiledSqlStatement
  compileOracle(
    operation: QueryOperation<Definition>,
    options?: OracleCompileOptions,
  ): import('../native.js').CompiledSqlStatement
}

export type DefinitionOf<Subject extends GraphDefinitionInput | QueryGraph | RelationalQueryGraph> =
  Subject extends GraphDefinitionInput
    ? Subject
    : Subject extends QueryGraph<infer Definition>
      ? Definition
      : Subject extends RelationalQueryGraph<infer Definition>
        ? Definition
        : never

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
