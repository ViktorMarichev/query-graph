# Authoring DSL

Authoring API находится в <code>@query-graph/core/dsl</code>. Он создает
семантический definition версии 10, который не содержит таблиц, SQL aliases или
синтаксиса диалекта.

## Sources и поля

```ts
import { fieldType, hidden, nullable, source } from '@query-graph/core/dsl'

const users = source('users', {
  id: 'int64',
  email: nullable('string'),
  internalRank: hidden(fieldType('int32', { nullable: true })),
})
```

Поле имеет scalar type, nullability и флаг selectable. Hidden-поля можно
использовать в constraints и relations, но нельзя вывести в projection.

## Параметры

```ts
const id = requiredParameter('id', 'int64')
const status = optionalParameter('status', 'string')
const ids = requiredListParameter('ids', 'int64')
```

Scalar parameter используется через <code>param</code>, list parameter через
<code>inParameter</code>. Optional parameter не активирует constraint с
<code>when</code>, пока значение отсутствует.

## Expressions

DSL предоставляет сравнения, boolean groups, null tests, semantic functions,
агрегаты и literals:

```ts
const predicate = and(eq(users.field('organisationId'), param(organisationId)), isNull(users.field('dateDelete')))
```

Типы operands проверяются TypeScript во время authoring и повторно Rust при
регистрации.

## Relations

```ts
const profileRelation = relation({
  name: 'profile',
  from: users,
  to: profiles,
  on: eq(users.field('id'), profiles.field('userId')),
  cardinality: 'one',
  required: false,
  selection: firstBy(asc(profiles.field('id'))),
})
```

- <code>cardinality</code> описывает форму связи: <code>one</code> или
  <code>many</code>.
- <code>required</code> определяет, может ли отсутствие связанной строки удалить
  root из результата.
- <code>firstBy</code> детерминированно выбирает одну строку среди кандидатов и
  компилируется через dialect-specific APPLY/LATERAL семантику.

У каждого source должен быть единственный путь от root. Planner использует этот
инвариант для минимального набора JOIN.

## Constraints и exists

```ts
constraint({
  when: organisationId,
  predicate: eq(users.field('organisationId'), param(organisationId)),
})

constraint({
  predicate: exists(memberships, eq(memberships.field('active'), true)),
})
```

<code>exists</code> является semijoin: он фильтрует множество root и не
материализует many relation. В relation predicate корреляция задается явно:

```ts
exists(flags, flagPredicate, { from: profiles })
```

## Projection и presence

```ts
const projection = [
  project({ path: 'id', expression: users.field('id'), default: true }),
  projectObject({ path: 'profile', presence: profiles.field('id') }),
  project({
    path: 'profile.name',
    expression: profiles.field('name'),
    default: true,
  }),
]
```

<code>projectObject</code> позволяет executor отличить отсутствующий to-one
объект от существующего объекта, все поля которого равны NULL.

## Ordering

```ts
const byName = ordering({
  name: 'nameAsc',
  by: [asc(users.field('name'), { nulls: 'last' })],
  default: true,
})
```

Operation выбирает ordering по имени. SQL expression остается частью
definition, поэтому потребитель не передает произвольный ORDER BY.

## GraphModule

<code>defineGraphModule</code> объединяет sources, parameters, relations,
constraints, projection objects и orderings без собственного root:

```ts
const auditModule = defineGraphModule({
  name: 'audit',
  sources: [audit],
  relations: [auditRelation],
  projection: [createdAtProjection],
})

const definition = defineGraph({
  name: 'users',
  root: users,
  modules: [auditModule],
  sources: [users],
  projection: [idProjection],
})
```

Повторно подключенный тот же constraint object дедуплицируется по identity.
Конфликтующие sources, parameters, relations и projection paths отклоняются.

## Summary

<code>defineSummaryGraph</code> принимает dimensions и measures:

```ts
const summary = defineSummaryGraph({
  name: 'ordersByStatus',
  root: orders,
  sources: [orders],
  dimensions: [dimension({ path: 'status', expression: orders.field('status'), default: true })],
  measures: [measure({ path: 'count', expression: count(), default: true })],
})
```

Dimensions становятся GROUP BY, aggregate constraints становятся HAVING. Rust
отклоняет nested aggregates, неагрегированные поля вне dimensions и независимые
many-ветви, которым нужен отдельный aggregate subquery plan.
