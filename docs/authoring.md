# Authoring DSL

Authoring API находится в <code>@query-graph/core/dsl</code>. Он создает
семантический definition версии 12, который не содержит таблиц, SQL aliases или
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
const id = requiredParam('id', 'int64')
const status = optionalParam('status', 'string')
const ids = requiredListParam('ids', 'int64')
const optionalIds = optionalListParam('optionalIds', 'int64')
```

Scalar parameter передается в expression напрямую, list parameter используется
через <code>inParam</code>. Явная форма <code>param(id)</code> поддерживается
для совместимости. Полные имена <code>requiredParameter</code>,
<code>optionalParameter</code>, <code>requiredListParameter</code>,
<code>optionalListParameter</code> и <code>inParameter</code> остаются алиасами
тех же factories. Optional parameter не активирует constraint с
<code>when</code>, пока значение отсутствует.

### Группированные параметры

Grouped list parameter описывает массив групп <code>{ key, values }</code>.
Каждая группа содержит один ключ и список значений:

```ts
import { allGroups, and, constraint, eq, inParam, optionalGroupsParam, source } from '@query-graph/core/dsl'

const attributes = source('attributes', { name: 'string', valueId: 'int64' })
const filters = optionalGroupsParam('filters', { key: 'string', value: 'int64' })
const attributeFilter = constraint({
  predicate: allGroups(filters, ({ key, values }) =>
    and(eq(attributes.field('name'), key), inParam(attributes.field('valueId'), values)),
  ),
})

// Operation parameters:
const parameters = {
  filters: [
    { key: 'color', values: [1, 2] },
    { key: 'size', values: [3] },
  ],
}
```

Добавьте <code>filters</code> в <code>parameters</code> definition, а
<code>attributeFilter</code> в <code>constraints</code>. Если разные группы
должны совпасть с разными связанными строками, поместите <code>exists</code>
внутрь callback и объявите соответствующую relation.

<code>requiredGroupsParam</code> требует присутствия массива в operation.
Полные имена <code>optionalGroupsParameter</code> и
<code>requiredGroupsParameter</code> являются алиасами тех же factories.
Неизвестные поля конфигурации, scalar types и некорректные аргументы factories
отклоняются сразу при authoring.

Callback <code>allGroups</code> вызывается ровно один раз при построении AST,
а не для каждой runtime-группы. <code>key</code> является типизированным scalar
expression, <code>values</code> является отдельной scoped list reference только
для <code>inParam</code>/<code>inParameter</code>. Ни grouped parameter, ни
<code>values</code> нельзя передать в <code>param</code> или использовать как
обычное scalar expression. <code>values</code> также нельзя объявить root parameter.

Каждый binder получает UUID scope через <code>node:crypto.randomUUID()</code>,
независимо от загрузки отдельных экземпляров или копий DSL. Вложенный callback
может захватить <code>key</code> и <code>values</code> внешнего callback;
одноименные параметры не смешивают их scopes. Ссылки за пределами своего binder,
в том числе захваченные из соседнего callback, отклоняются Rust при регистрации.
Scope является внутренним идентификатором, его конкретное значение не гарантируется.

Семантика компиляции:

- отсутствующий optional parameter и пустой массив групп дают <code>true</code>;
- неактивное тело не требует optional scalar parameters или JOIN, используемых
  только внутри него;
- predicates всех групп объединяются через <code>AND</code>;
- повторяющиеся ключи остаются независимыми группами, их values не объединяются;
- пустой <code>values</code> дает <code>false</code> в <code>inParam</code>;
- nullable predicate сохраняет обычную трехзначную SQL-семантику.

Rust разрешает <code>allGroups</code> только в constraint predicates, включая
вложенные boolean expressions и <code>exists</code>. Projection, ordering,
relation predicates и aggregate bodies не поддерживают binder.

Wire definition сохраняет тип элементов в <code>scalarType</code> и добавляет
<code>shape: 'groupedList'</code> и <code>keyType</code>. AST использует
<code>{ kind: 'allGroups', parameter, scope, predicate }</code>,
<code>{ kind: 'groupKey', scope }</code> и
<code>{ kind: 'inGroupValues', expression, scope }</code>.

## Expressions

DSL предоставляет сравнения, boolean groups, null tests, semantic functions,
агрегаты и literals:

```ts
const predicate = and(eq(users.field('organisationId'), organisationId), isNull(users.field('dateDelete')))
```

Semantic functions позволяют оставить вычисляемые поля частью графа, не
привязывая definition к синтаксису Oracle или SQL Server:

```ts
const durationProjection = [
  project({
    path: 'hoursDuration',
    expression: truncate(divide(services.field('durationMinutes'), 60)),
  }),
  project({
    path: 'minutesDuration',
    expression: modulo(services.field('durationMinutes'), 60),
  }),
]
```

<code>divide</code> возвращает decimal для точных числовых типов и float64,
если хотя бы один operand имеет тип float64. <code>truncate</code> отбрасывает
дробную часть по направлению к нулю и сохраняет scalar type. <code>modulo</code>
принимает int32, int64 и decimal; float64 отклоняется, чтобы definition одинаково
работал в поддерживаемых диалектах. Nullability распространяется от operands к
результату.

TypeScript выводит тип и nullability результата во время authoring. Rust при
регистрации повторно проверяет arity и совместимость типов operands.

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
  predicate: eq(users.field('organisationId'), organisationId),
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
