import test from 'ava'
import { field, project as positionalProject, relation as positionalRelation } from '../definition.js'

import { registerDefinition } from '../index.js'
import {
  count,
  coalesce,
  defineGraph,
  defineGraphModule,
  defineSummaryGraph,
  dimension,
  eq,
  fieldType,
  literal,
  measure,
  neq,
  nullable,
  project,
  projectObject,
  relation,
  source,
  sum,
} from '../dsl.js'
import type {
  Expression,
  FieldSpecIsNullable,
  FieldTypeOptions,
  QueryOperation,
  ResultOf,
  ScalarOutputTypeMap,
} from '../dsl.js'

type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <Value>() => Value extends Right ? 1 : 2 ? true : false

const attachment = source('attachment', {
  id: 'int64',
  description: nullable('string'),
  createdAt: 'dateTime',
})

const link = source('link', {
  id: 'int64',
  idAttachment: 'int64',
  idStorage: 'int64',
})

const storage = source('storage', {
  id: 'int64',
  name: 'string',
  size: nullable('string'),
})

const storageModule = defineGraphModule({
  name: 'storageDetails',
  sources: [link, storage],
  relations: [
    relation({
      name: 'storage',
      from: link,
      to: storage,
      required: true,
      on: eq(link.field('idStorage'), storage.field('id')),
    }),
  ],
  projection: [
    project({ path: 'links.idStorage', expression: link.field('idStorage') }),
    project({ path: 'links.storage.name', expression: storage.field('name') }),
    project({ path: 'links.storage.size', expression: storage.field('size') }),
    project({
      path: 'links.storage.displayName',
      expression: coalesce(storage.field('name'), 'Unavailable'),
    }),
  ],
})

const definition = defineGraph({
  name: 'attachments',
  root: attachment,
  modules: [storageModule],
  sources: [attachment],
  relations: [
    relation({
      name: 'links',
      from: attachment,
      to: link,
      cardinality: 'many',
      on: eq(attachment.field('id'), link.field('idAttachment')),
    }),
  ],
  projection: [
    project({ path: 'idAttachment', expression: attachment.field('id'), default: true }),
    project({ path: 'description', expression: attachment.field('description'), default: true }),
    project({ path: 'createdAt', expression: attachment.field('createdAt') }),
  ],
})

test('infers default projection result types', (t) => {
  type Actual = ResultOf<typeof definition>
  type Expected = {
    idAttachment: number | string
    description: string | null
  }

  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)
})

test('preserves possible nullability from widened field options', (t) => {
  const widened: { nullable: boolean } = { nullable: true }
  const optional: { nullable?: boolean } = { nullable: true }
  const broad: FieldTypeOptions = { nullable: true }
  const readonly: Readonly<{ nullable: boolean }> = { nullable: true }
  const union = (): { nullable: true } | { nullable: false } => ({ nullable: true })
  const unionWithOmission = (): { nullable: true } | Record<never, never> => ({ nullable: true })
  const optionalTrue: { nullable?: true } = {}
  const optionalFalse: { nullable?: false } = {}
  const fields = {
    omitted: fieldType('string'),
    empty: fieldType('string', {}),
    explicitTrue: fieldType('string', { nullable: true }),
    explicitFalse: fieldType('string', { nullable: false }),
    widened: fieldType('string', widened),
    optional: fieldType('string', optional),
    broad: fieldType('string', broad),
    readonly: fieldType('string', readonly),
    union: fieldType('string', union()),
    unionWithOmission: fieldType('string', unionWithOmission()),
    optionalTrue: fieldType('string', optionalTrue),
    optionalFalse: fieldType('string', optionalFalse),
    explicitUndefined: fieldType('string', { nullable: undefined }),
  }
  type ActualNullability = { [Key in keyof typeof fields]: FieldSpecIsNullable<(typeof fields)[Key]> }
  type ExpectedNullability = {
    omitted: false
    empty: false
    explicitTrue: true
    explicitFalse: false
    widened: boolean
    optional: boolean
    broad: boolean
    readonly: boolean
    union: boolean
    unionWithOmission: boolean
    optionalTrue: boolean
    optionalFalse: false
    explicitUndefined: false
  }
  const optionsAreInferred: Equal<ActualNullability, ExpectedNullability> = true
  t.true(optionsAreInferred)

  const item = source('item', fields)
  const definition = defineGraph({
    name: 'fieldOptions',
    root: item,
    sources: [item],
    projection: [
      project({ path: 'widened', expression: item.field('widened'), default: true }),
      project({ path: 'optional', expression: item.field('optional'), default: true }),
      project({ path: 'broad', expression: item.field('broad'), default: true }),
      project({ path: 'readonly', expression: item.field('readonly'), default: true }),
      project({ path: 'union', expression: item.field('union'), default: true }),
      project({ path: 'unionWithOmission', expression: item.field('unionWithOmission'), default: true }),
    ],
  })
  type Actual = ResultOf<typeof definition>
  type Expected = {
    widened: string | null
    optional: string | null
    broad: string | null
    readonly: string | null
    union: string | null
    unionWithOmission: string | null
  }
  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)
  const unsafeAssumption = (row: Actual): string => {
    // @ts-expect-error A runtime boolean option can make this field nullable.
    return row.widened
  }
  t.is(typeof unsafeAssumption, 'function')

  const graph = registerDefinition(definition).withRelationalMapping({ sources: { item: { table: 'Item' } } })
  for (const statement of [graph.compileSqlServer({}), graph.compileOracle({})]) {
    t.true(statement.columns.every(({ nullable }) => nullable))
  }
  t.deepEqual(Object.fromEntries(Object.entries(fields).map(([name, field]) => [name, field.nullable === true])), {
    omitted: false,
    empty: false,
    explicitTrue: true,
    explicitFalse: false,
    widened: true,
    optional: true,
    broad: true,
    readonly: true,
    union: true,
    unionWithOmission: true,
    optionalTrue: false,
    optionalFalse: false,
    explicitUndefined: false,
  })
})

test('distinguishes possible null operands from literal null in equality result types', (t) => {
  const item = source('equalityItem', { id: 'int64', nullableName: nullable('string'), name: 'string' })
  const profile = source('equalityProfile', { idItem: 'int64', name: 'string' })
  const maybeText = (): string | null => 'Ada'

  const checkUntypedExpressions = (left: Expression, right: Expression) => {
    const definition = defineGraph({
      name: 'untypedEquality',
      root: item,
      sources: [item],
      projection: [
        project({ path: 'equals', expression: eq(left, right), default: true }),
        project({ path: 'differs', expression: neq(left, right), default: true }),
        project({ path: 'equalsNull', expression: eq(left, null), default: true }),
        project({ path: 'differsNull', expression: neq(null, right), default: true }),
      ],
    })
    type Expected = { equals: boolean | null; differs: boolean | null; equalsNull: boolean; differsNull: boolean }
    const resultIsInferred: Equal<ResultOf<typeof definition>, Expected> = true
    return resultIsInferred
  }
  t.is(typeof checkUntypedExpressions, 'function')

  for (const compare of [eq, neq]) {
    const definition = defineGraph({
      name: 'equalityNullability',
      root: item,
      sources: [item, profile],
      relations: [
        relation({ name: 'profile', from: item, to: profile, on: eq(item.field('id'), profile.field('idItem')) }),
      ],
      projection: [
        project({ path: 'nullableLeft', expression: compare(item.field('nullableName'), maybeText()), default: true }),
        project({ path: 'nullableRight', expression: compare(maybeText(), item.field('nullableName')), default: true }),
        project({
          path: 'wrappedLeft',
          expression: compare(item.field('nullableName'), literal(maybeText())),
          default: true,
        }),
        project({
          path: 'wrappedRight',
          expression: compare(literal(maybeText()), item.field('nullableName')),
          default: true,
        }),
        project({ path: 'nonnullLeft', expression: compare(item.field('name'), maybeText()), default: true }),
        project({ path: 'nonnullRight', expression: compare(maybeText(), item.field('name')), default: true }),
        project({
          path: 'nonnullWrappedLeft',
          expression: compare(item.field('name'), literal(maybeText())),
          default: true,
        }),
        project({
          path: 'nonnullWrappedRight',
          expression: compare(literal(maybeText()), item.field('name')),
          default: true,
        }),
        project({ path: 'nonnull', expression: compare(item.field('name'), 'Ada'), default: true }),
        project({ path: 'nullLeft', expression: compare(null, item.field('nullableName')), default: true }),
        project({ path: 'nullRight', expression: compare(item.field('nullableName'), null), default: true }),
        project({
          path: 'literalNullLeft',
          expression: compare(literal(null), item.field('nullableName')),
          default: true,
        }),
        project({
          path: 'literalNullRight',
          expression: compare(item.field('nullableName'), literal(null)),
          default: true,
        }),
        project({
          path: 'optionalRelationLeft',
          expression: compare(profile.field('name'), maybeText()),
          default: true,
        }),
        project({
          path: 'optionalRelationRight',
          expression: compare(maybeText(), profile.field('name')),
          default: true,
        }),
        project({ path: 'optionalRelationNull', expression: compare(profile.field('name'), null), default: true }),
      ],
    })
    type Actual = ResultOf<typeof definition>
    type Expected = {
      nullableLeft: boolean | null
      nullableRight: boolean | null
      wrappedLeft: boolean | null
      wrappedRight: boolean | null
      nonnullLeft: boolean
      nonnullRight: boolean
      nonnullWrappedLeft: boolean
      nonnullWrappedRight: boolean
      nonnull: boolean
      nullLeft: boolean
      nullRight: boolean
      literalNullLeft: boolean
      literalNullRight: boolean
      optionalRelationLeft: boolean | null
      optionalRelationRight: boolean | null
      optionalRelationNull: boolean
    }
    const resultIsInferred: Equal<Actual, Expected> = true
    t.true(resultIsInferred)
    const unsafeDirect = (row: Actual): boolean => {
      // @ts-expect-error A possibly non-null operand can produce SQL UNKNOWN against a nullable field.
      return row.nullableLeft
    }
    const unsafeWrapped = (row: Actual): boolean => {
      // @ts-expect-error Wrapping the possible null value as a literal must not discard nullability.
      return row.wrappedRight
    }
    t.is(typeof unsafeDirect, 'function')
    t.is(typeof unsafeWrapped, 'function')

    const graph = registerDefinition(definition).withRelationalMapping({
      sources: { equalityItem: { table: 'Item' }, equalityProfile: { table: 'Profile' } },
    })
    // Check inferred metadata only; these assertions do not validate boolean projection SQL on a database.
    for (const statement of [graph.compileSqlServer({}), graph.compileOracle({})]) {
      t.deepEqual(
        statement.columns.map(({ path, nullable }) => [path, nullable]),
        [
          ['nullableLeft', true],
          ['nullableRight', true],
          ['wrappedLeft', true],
          ['wrappedRight', true],
          ['nonnullLeft', false],
          ['nonnullRight', false],
          ['nonnullWrappedLeft', false],
          ['nonnullWrappedRight', false],
          ['nonnull', false],
          ['nullLeft', false],
          ['nullRight', false],
          ['literalNullLeft', false],
          ['literalNullRight', false],
          ['optionalRelationLeft', true],
          ['optionalRelationRight', true],
          ['optionalRelationNull', false],
        ],
      )
    }
  }
})

test('infers explicit nested selections and relation nullability', (t) => {
  const operation = {
    select: [
      'idAttachment',
      'createdAt',
      'links.idStorage',
      'links.storage.name',
      'links.storage.size',
      'links.storage.displayName',
    ],
  } as const satisfies QueryOperation<typeof definition>

  type Actual = ResultOf<typeof definition, typeof operation>
  type Expected = {
    idAttachment: number | string
    createdAt: string
    links: {
      idStorage: number | string | null
      storage: {
        name: string | null
        size: string | null
        displayName: string
      }
    }
  }

  const resultIsInferred: Equal<Actual, Expected> = true
  t.true(resultIsInferred)

  const graph = registerDefinition(definition)
  type RegisteredResult = ResultOf<typeof graph, typeof operation>
  const registeredGraphKeepsDefinition: Equal<RegisteredResult, Expected> = true
  t.true(registeredGraphKeepsDefinition)

  const relationalGraph = graph.withRelationalMapping({
    sources: {
      attachment: { table: 'Attachment' },
      link: { table: 'AttachmentStorageLink' },
      storage: { table: 'Storage' },
    },
  })
  type RelationalResult = ResultOf<typeof relationalGraph, typeof operation>
  const relationalGraphKeepsDefinition: Equal<RelationalResult, Expected> = true
  t.true(relationalGraphKeepsDefinition)
})

test('supports driver-specific scalar output types', (t) => {
  interface DriverScalarTypes extends ScalarOutputTypeMap {
    boolean: boolean
    int32: number
    int64: bigint
    float64: number
    decimal: string
    string: string
    date: Date
    dateTime: Date
    binary: Uint8Array
    json: unknown
  }

  const operation = {
    select: ['idAttachment', 'createdAt', 'links.idStorage'],
  } as const satisfies QueryOperation<typeof definition>

  type Actual = ResultOf<typeof definition, typeof operation, DriverScalarTypes>
  type Expected = {
    idAttachment: bigint
    createdAt: Date
    links: {
      idStorage: bigint | null
    }
  }

  const driverTypesAreApplied: Equal<Actual, Expected> = true
  t.true(driverTypesAreApplied)
})

test('infers result types through the positional DSL', (t) => {
  const account = source('account', { id: 'int64' })
  const profile = source('profile', {
    idAccount: 'int64',
    displayName: nullable('string'),
  })

  const positionalDefinition = defineGraph({
    name: 'accounts',
    root: account,
    sources: [account, profile],
    relations: [positionalRelation('profile', account, profile, eq(field(account, 'id'), field(profile, 'idAccount')))],
    projection: [
      positionalProject('id', field(account, 'id'), { default: true }),
      positionalProject('profile.displayName', field(profile, 'displayName'), { default: true }),
    ],
  })

  type Actual = ResultOf<typeof positionalDefinition>
  type Expected = {
    id: number | string
    profile: {
      displayName: string | null
    }
  }

  const positionalTypesAreInferred: Equal<Actual, Expected> = true
  t.true(positionalTypesAreInferred)
})

test('infers summary dimensions and measures', (t) => {
  const invoice = source('invoice', {
    id: 'int64',
    idCustomer: 'int64',
    total: 'decimal',
  })

  const summaryDefinition = defineSummaryGraph({
    name: 'invoiceSummary',
    root: invoice,
    sources: [invoice],
    dimensions: [dimension({ path: 'customerId', expression: invoice.field('idCustomer'), default: true })],
    measures: [
      measure({ path: 'invoiceCount', expression: count(invoice.field('id')), default: true }),
      measure({ path: 'total', expression: sum(invoice.field('total')), default: true }),
    ],
  })

  type Actual = ResultOf<typeof summaryDefinition>
  type Expected = {
    customerId: number | string
    invoiceCount: number | string
    total: number | string | null
  }

  const summaryTypesAreInferred: Equal<Actual, Expected> = true
  t.true(summaryTypesAreInferred)
})

test('infers nullable projection objects from their presence expression', (t) => {
  const account = source('objectAccount', { id: 'int64' })
  const profile = source('objectProfile', {
    id: 'int64',
    idAccount: 'int64',
    displayName: nullable('string'),
  })

  const profileModule = defineGraphModule({
    name: 'accountProfile',
    sources: [profile],
    objects: [projectObject({ path: 'profile', presence: profile.field('id') })],
    projection: [
      project({
        path: 'profile.displayName',
        expression: profile.field('displayName'),
        default: true,
      }),
    ],
  })

  const objectDefinition = defineGraph({
    name: 'accountsWithProfiles',
    root: account,
    modules: [profileModule],
    sources: [account],
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

  type Actual = ResultOf<typeof objectDefinition>
  type Expected = {
    id: number | string
    profile: {
      displayName: string | null
    } | null
  }

  const objectNullabilityIsInferred: Equal<Actual, Expected> = true
  t.true(objectNullabilityIsInferred)
})
