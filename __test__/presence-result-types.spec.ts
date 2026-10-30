import test from 'ava'

import { registerDefinition } from '../index.js'
import {
  batchQuery,
  batchRelation,
  coalesce,
  composeGraph,
  defineGraph,
  defineGraphModule,
  eq,
  isNotNull,
  nullable,
  optionalListParam,
  project,
  projectObject,
  relation,
  source,
  upper,
} from '../dsl.js'
import type {
  DefaultScalarOutputTypeMap,
  Expression,
  GraphDefinitionInput,
  ProjectionObjectDefinition,
  ResultOf,
  SourceRef,
} from '../dsl.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const account = source('account', { id: 'int64' })
const profile = source('profile', { id: 'int64', idAccount: 'int64', name: 'string', nickname: nullable('string') })
const settings = source('settings', { id: 'int64', idProfile: 'int64', label: 'string', note: nullable('string') })
const ids = optionalListParam('ids', 'int64')

function defineProfiles<const Objects extends readonly ProjectionObjectDefinition[]>(objects: Objects) {
  const profileModule = defineGraphModule({
    name: 'profileDetails',
    sources: [profile, settings],
    relations: [
      relation({
        name: 'settings',
        from: profile,
        to: settings,
        required: true,
        on: eq(profile.field('id'), settings.field('idProfile')),
      }),
    ],
    objects,
    projection: [
      project({ path: 'profileId', expression: profile.field('id'), default: true }),
      project({ path: 'profile.id', expression: profile.field('id'), default: true }),
      project({ path: 'profile.name', expression: profile.field('name'), default: true }),
      project({ path: 'profile.nickname', expression: profile.field('nickname'), default: true }),
      project({ path: 'profile.upperName', expression: upper(profile.field('name')) }),
      project({ path: 'profile.fallbackName', expression: coalesce(profile.field('name'), profile.field('nickname')) }),
      project({ path: 'profile.equals', expression: eq(profile.field('name'), 'Ada') }),
      project({ path: 'profile.nicknameEquals', expression: eq(profile.field('nickname'), 'Ada') }),
      project({ path: 'profile.combined', expression: eq(profile.field('name'), settings.field('label')) }),
      project({ path: 'profile.settingsLabel', expression: settings.field('label') }),
      project({ path: 'profile.sameSource.name', expression: profile.field('name') }),
      project({ path: 'profile.settings.id', expression: settings.field('id') }),
      project({ path: 'profile.settings.label', expression: settings.field('label') }),
      project({ path: 'profile.settings.note', expression: settings.field('note') }),
      project({ path: 'profile.settings.profileName', expression: profile.field('name') }),
      project({ path: 'settings.profileName', expression: profile.field('name') }),
    ],
  })
  return defineGraph({
    name: 'accounts',
    root: account,
    sources: [account],
    parameters: [ids],
    modules: [profileModule],
    relations: [
      relation({
        name: 'profile',
        from: account,
        to: profile,
        on: eq(account.field('id'), profile.field('idAccount')),
      }),
    ],
    projection: [project({ path: 'id', expression: account.field('id'), default: true })],
  })
}

const definition = defineProfiles([projectObject({ path: 'profile', presence: profile.field('id') })])
const registered = registerDefinition(definition)
const graph = registered.withRelationalMapping({
  sources: { account: { table: 'Account' }, profile: { table: 'Profile' }, settings: { table: 'Settings' } },
})

test('refines fields inside a present object without changing raw SQL column nullability', (t) => {
  type Expected = {
    id: number | string
    profileId: number | string | null
    profile: { id: number | string; name: string; nickname: string | null } | null
  }
  const definitionResult: Equal<ResultOf<typeof definition>, Expected> = true
  const registeredResult: Equal<ResultOf<typeof registered>, Expected> = true
  const mappedResult: Equal<ResultOf<typeof graph>, Expected> = true
  t.true(definitionResult && registeredResult && mappedResult)

  type Selection = { select: readonly ['profile.name'] }
  const selectedResult: Equal<ResultOf<typeof graph, Selection>, { profile: { name: string } | null }> = true
  t.true(selectedResult)

  interface DriverScalars extends Omit<DefaultScalarOutputTypeMap, 'int64'> {
    int64: bigint
  }
  type DriverExpected = {
    id: bigint
    profileId: bigint | null
    profile: { id: bigint; name: string; nickname: string | null } | null
  }
  const driverResult: Equal<ResultOf<typeof graph, undefined, DriverScalars>, DriverExpected> = true
  t.true(driverResult)

  const checkResult = (row: ResultOf<typeof graph>) => {
    // @ts-expect-error The object itself still needs a presence check.
    const unsafeName: string = row.profile.name
    if (row.profile) {
      const name: string = row.profile.name
      // @ts-expect-error Source column nullability is not removed by object presence.
      const nickname: string = row.profile.nickname
      return { name, nickname }
    }
    return unsafeName
  }
  t.is(typeof checkResult, 'function')

  for (const statement of [graph.compileOracle({}), graph.compileSqlServer({})]) {
    t.deepEqual(
      statement.columns.map(({ path, nullable }) => [path, nullable]),
      [
        ['profileId', true],
        ['profile.id', true],
        ['profile.name', true],
        ['profile.nickname', true],
        ['id', false],
      ],
    )
    t.is(statement.objects.length, 1)
    t.is(statement.objects[0].path, 'profile')
    t.true(statement.sql.includes('LEFT JOIN'))
  }
})

test('evaluates expression nullability with presence evidence only for the matching source', (t) => {
  type Selection = {
    select: readonly [
      'profile.upperName',
      'profile.fallbackName',
      'profile.equals',
      'profile.nicknameEquals',
      'profile.combined',
      'profile.settingsLabel',
      'profile.sameSource.name',
    ]
  }
  type Expected = {
    profile: {
      upperName: string
      fallbackName: string
      equals: boolean
      nicknameEquals: boolean | null
      combined: boolean | null
      settingsLabel: string | null
      sameSource: { name: string }
    } | null
  }
  const result: Equal<ResultOf<typeof graph, Selection>, Expected> = true
  t.true(result)
})

test('inherits outer presence without making optional nested objects or ancestors present', (t) => {
  const nested = defineProfiles([
    projectObject({ path: 'profile', presence: profile.field('id') }),
    projectObject({ path: ['profile', 'sameSource'], presence: profile.field('id') }),
    projectObject({ path: 'profile.settings', presence: settings.field('id') }),
    projectObject({ path: 'settings', presence: settings.field('id') }),
  ])
  type Selection = {
    select: readonly [
      'profile.sameSource.name',
      'profile.settings.id',
      'profile.settings.label',
      'profile.settings.note',
      'profile.settings.profileName',
      'settings.profileName',
    ]
  }
  type Expected = {
    profile: {
      sameSource: { name: string }
      settings: { id: number | string; label: string; note: string | null; profileName: string } | null
    } | null
    settings: { profileName: string | null } | null
  }
  const result: Equal<ResultOf<typeof nested, Selection>, Expected> = true
  t.true(result)
})

test('does not infer source presence from computed expressions', (t) => {
  const fallback = defineProfiles([projectObject({ path: 'profile', presence: coalesce(profile.field('id'), 0) })])
  const predicate = defineProfiles([projectObject({ path: 'profile', presence: isNotNull(profile.field('id')) })])
  const transformed = defineProfiles([projectObject({ path: 'profile', presence: upper(profile.field('name')) })])
  const nullableField = defineProfiles([projectObject({ path: 'profile', presence: profile.field('nickname') })])
  type Selection = { select: readonly ['profile.name', 'profile.nickname'] }
  type UnknownFields = { name: string | null; nickname: string | null }
  const fallbackResult: Equal<ResultOf<typeof fallback, Selection>, { profile: UnknownFields }> = true
  const predicateResult: Equal<ResultOf<typeof predicate, Selection>, { profile: UnknownFields }> = true
  const transformedResult: Equal<ResultOf<typeof transformed, Selection>, { profile: UnknownFields | null }> = true
  const nullableFieldResult: Equal<
    ResultOf<typeof nullableField, Selection>,
    { profile: { name: string; nickname: string | null } | null }
  > = true
  t.true(fallbackResult && predicateResult && transformedResult && nullableFieldResult)
})

test('keeps widened and ambiguous presence declarations conservative', (t) => {
  const check = (
    expression: Expression,
    broadSource: SourceRef<string, { id: 'int64' }>,
    chooseProfile: boolean,
    path: string,
    unionPath: 'profile' | 'other',
  ) => {
    const broadExpression = defineProfiles([projectObject({ path: 'profile', presence: expression })])
    const broadKey = defineProfiles([projectObject({ path: 'profile', presence: broadSource.field('id') })])
    const unionSource = defineProfiles([
      projectObject({ path: 'profile', presence: chooseProfile ? profile.field('id') : settings.field('id') }),
    ])
    const unionExpression = defineProfiles([
      projectObject({
        path: 'profile',
        presence: chooseProfile ? profile.field('id') : coalesce(profile.field('id'), 0),
      }),
    ])
    const unionObject = defineProfiles([
      chooseProfile
        ? projectObject({ path: 'profile', presence: profile.field('id') })
        : projectObject({ path: 'profile', presence: coalesce(profile.field('id'), 0) }),
    ])
    const unionObjectSource = defineProfiles([
      chooseProfile
        ? projectObject({ path: 'profile', presence: profile.field('id') })
        : projectObject({ path: 'profile', presence: settings.field('id') }),
    ])
    const broadPath = defineProfiles([projectObject({ path, presence: profile.field('id') })])
    const ambiguousPath = defineProfiles([projectObject({ path: unionPath, presence: profile.field('id') })])
    const untypedObject: ProjectionObjectDefinition<'profile'> = { path: ['profile'], presence: profile.field('id') }
    const untyped = defineProfiles([untypedObject])
    type Selection = { select: readonly ['profile.name'] }
    type FieldOf<Graph extends GraphDefinitionInput> =
      ResultOf<Graph, Selection> extends { profile: infer Profile }
        ? NonNullable<Profile> extends { name: infer Name }
          ? Name
          : never
        : never
    const broadExpressionResult: Equal<FieldOf<typeof broadExpression>, string | null> = true
    const broadKeyResult: Equal<FieldOf<typeof broadKey>, string | null> = true
    const unionSourceResult: Equal<FieldOf<typeof unionSource>, string | null> = true
    const unionExpressionResult: Equal<FieldOf<typeof unionExpression>, string | null> = true
    const unionObjectResult: Equal<FieldOf<typeof unionObject>, string | null> = true
    const unionObjectSourceResult: Equal<FieldOf<typeof unionObjectSource>, string | null> = true
    const broadPathResult: Equal<NonNullable<ResultOf<typeof broadPath, Selection>['profile']>['name'], string | null> =
      true
    const ambiguousPathResult: Equal<
      NonNullable<ResultOf<typeof ambiguousPath, Selection>['profile']>['name'],
      string | null
    > = true
    const untypedResult: Equal<NonNullable<ResultOf<typeof untyped, Selection>['profile']>['name'], string | null> =
      true
    return [
      broadExpressionResult,
      broadKeyResult,
      unionSourceResult,
      unionExpressionResult,
      unionObjectResult,
      unionObjectSourceResult,
      broadPathResult,
      ambiguousPathResult,
      untypedResult,
    ]
  }
  t.is(typeof check, 'function')
})

test('preserves presence-aware results through batch composition', (t) => {
  const query = batchQuery({ graph, key: { path: 'id', parameter: ids } })
  const composed = composeGraph({
    root: graph,
    relations: [
      batchRelation({ name: 'related', from: 'id', query, cardinality: 'many' }),
      batchRelation({ name: 'primary', from: 'id', query, cardinality: 'one' }),
    ],
  })
  type Selection = { select: readonly ['profile.name', 'related.profile.name', 'primary.profile.nickname'] }
  type Row = ResultOf<typeof composed, Selection>
  const root: Equal<Row['profile'], { name: string } | null> = true
  const many: Equal<Row['related'], { profile: { name: string } | null }[]> = true
  const one: Equal<Row['primary'], { profile: { nickname: string | null } | null } | null> = true
  t.true(root && many && one)
})
