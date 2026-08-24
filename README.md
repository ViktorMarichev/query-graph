# query-graph

Типизированное описание графов запросов, нативное планирование и компиляция SQL
для Node.js.

Граф описывает доступные данные, связи, ограничения и форму результата без
синтаксиса конкретной СУБД. Rust валидирует definition, выбирает минимальный
набор связей и компилирует operation в SQL Server или Oracle SQL.

Пакет не подключается к базе данных и не выполняет запросы. На выходе он
возвращает SQL, bindings и metadata, необходимые executor приложения.

## Возможности

- объектный TypeScript DSL с выводом параметров, projection paths и результата;
- строгая runtime-валидация definition, mapping и operation в Rust;
- выбор JOIN только для реально запрошенных полей и условий;
- scalar и list parameters без подстановки значений в SQL;
- <code>exists</code>, <code>firstBy</code>, nullable projection objects и именованные ordering;
- summary-графы с dimensions, measures, <code>GROUP BY</code> и <code>HAVING</code>;
- batch relations для загрузки коллекций после пагинации root;
- один семантический definition для SQL Server и Oracle.

Требуется Node.js 20 или новее.

## Установка

```bash
npm install @query-graph/core
```

Используйте два entrypoint:

- <code>@query-graph/core/dsl</code> для описания графа и TypeScript типов;
- <code>@query-graph/core</code> для регистрации и нативной компиляции.

## Быстрый старт

```ts
import { registerDefinition } from '@query-graph/core'
import { constraint, defineGraph, eq, project, requiredParam, source } from '@query-graph/core/dsl'
import type { ResultOf } from '@query-graph/core/dsl'

const users = source('users', {
  id: 'int64',
  organisationId: 'int64',
  email: 'string',
})

const organisationId = requiredParam('organisationId', 'int64')

const usersDefinition = defineGraph({
  name: 'users',
  root: users,
  sources: [users],
  parameters: [organisationId],
  constraints: [
    constraint({
      predicate: eq(users.field('organisationId'), organisationId),
    }),
  ],
  projection: [
    project({ path: 'id', expression: users.field('id'), default: true }),
    project({ path: 'email', expression: users.field('email'), default: true }),
  ],
})

const usersGraph = registerDefinition(usersDefinition).withRelationalMapping({
  sources: {
    users: {
      table: { schema: 'dbo', name: 'users' },
      columns: {
        organisationId: 'organisation_id',
      },
    },
  },
})

const statement = usersGraph.compileSqlServer({ parameters: { organisationId: 42 } }, { version: '2022' })
type UserRow = ResultOf<typeof usersDefinition>
```

<code>statement</code> содержит:

| Поле                   | Назначение                                                                        |
| ---------------------- | --------------------------------------------------------------------------------- |
| <code>sql</code>       | параметризованный SQL с placeholders <code>p0</code>, <code>p1</code> и так далее |
| <code>bindings</code>  | связь placeholder с operation parameter и типом                                   |
| <code>columns</code>   | связь SQL alias <code>cN</code> с логическим projection path                      |
| <code>objects</code>   | presence metadata для nullable вложенных объектов                                 |
| <code>relations</code> | связи, выбранные planner для этой operation                                       |

Executor берет значения из <code>parameters</code> переданной operation по
<code>bindings</code>, вызывает DB driver и восстанавливает объект по
<code>columns</code> и <code>objects</code>. Разбирать SQL не нужно.

Тот же mapped graph можно скомпилировать в Oracle:

```ts
const oracleStatement = usersGraph.compileOracle({ parameters: { organisationId: 42 } }, { version: '19c' })
```

## Зачем нужен planner

В реальном графе преимущество проявляется, когда одна definition обслуживает
разные формы результата:

```text
orders
|-- customer
|   `-- organisation
|-- latestPayment [firstBy]
`-- items [batch many]

constraints: organisationId, exists(activePromotion)
```

<code>ordersDefinition</code> содержит показанные на схеме relations и
projection. Его небанальные части выглядят так:

```ts
const latestPayment = relation({
  name: 'latestPayment',
  from: orders,
  to: payments,
  on: eq(orders.field('id'), payments.field('orderId')),
  selection: firstBy(desc(payments.field('createdAt'))),
})

const hasActivePromotion = constraint({
  predicate: exists(promotions, eq(promotions.field('active'), true)),
})

const ordersDefinition = defineGraph({
  name: 'orders',
  root: orders,
  sources: [orders, customers, organisations, payments, promotions],
  parameters: [organisationId],
  relations: [customer, customerOrganisation, latestPayment, activePromotion],
  constraints: [byOrganisation, hasActivePromotion],
  objects: [customerObject, latestPaymentObject],
  projection: [orderId, customerName, customerOrganisationName, latestPaymentAmount],
})

const items = batchRelation({
  name: 'items',
  from: 'id',
  query: batchQuery({
    graph: orderItemsGraph,
    key: { path: 'orderId', parameter: orderIds },
  }),
  cardinality: 'many',
})

const mappedOrders = registerDefinition(ordersDefinition).withRelationalMapping(orderMapping)
const ordersGraph = composeGraph({ root: mappedOrders, relations: [items] })
```

Обычные source и projection declarations здесь свернуты в именованные
константы. <code>customerObject</code> и <code>latestPaymentObject</code> созданы
через <code>projectObject</code>, поэтому отсутствующая to-one строка становится
<code>null</code>.

Одна и та же definition теперь обслуживает дешевый список:

```ts
const listPlan = ordersGraph.compileSqlServerPlan({
  select: ['id'],
  parameters: { organisationId: 42 },
})

listPlan.root.relations // []
listPlan.batches // []
```

И подробное представление:

```ts
const detailsPlan = ordersGraph.compileSqlServerPlan({
  select: ['id', 'customer.name', 'latestPayment.amount', 'items.name'],
  parameters: { organisationId: 42 },
})

detailsPlan.root.relations.map(({ name }) => name)
// ['customer', 'latestPayment']

detailsPlan.batches.map(({ name }) => name)
// ['items']
```

Planner не добавляет невыбранную <code>customerOrganisation</code>,
<code>exists(activePromotion)</code> остается semijoin без размножения root-строк,
а <code>items</code> загружается одним batch-запросом для всех найденных orders.

## Модель

```text
defineGraph
    |
    v
registerDefinition        Rust validation + reusable analysis
    |
    v
withRelationalMapping     logical sources -> physical tables
    |
    +---- QueryOperation
    |          |
    v          v
compileSqlServer / compileOracle
    |
    v
CompiledSqlStatement      SQL + bindings + result metadata
    |
    v
application executor      DB connection remains outside query-graph
```

| Понятие                           | Ответственность                                                    |
| --------------------------------- | ------------------------------------------------------------------ |
| <code>GraphDefinition</code>      | sources, parameters, relations, constraints, projection и ordering |
| <code>GraphModule</code>          | переиспользуемая часть definition без собственного root            |
| <code>QueryGraph</code>           | проверенный и проиндексированный нативный граф                     |
| <code>RelationalMapping</code>    | таблицы, схемы и физические имена колонок                          |
| <code>QueryOperation</code>       | select, parameters, ordering и pagination одного запроса           |
| <code>CompiledSqlStatement</code> | SQL и контракт результата для executor                             |

## Куда дальше

- [Authoring DSL](docs/authoring.md): sources, expressions, relations,
  constraints, projections, ordering и summary.
- [Composition](docs/composition.md): GraphModule, batchQuery, batchRelation и
  двухфазное выполнение.
- [Type system](docs/type-system.md): scalar types, nullability,
  <code>QueryOperation</code> и <code>ResultOf</code>.
- [SQL compilation](docs/sql.md): relational mapping, dialect versions,
  bindings и контракт executor.
- [Diagnostics](docs/diagnostics.md): фазы ошибок, коды и границы runtime
  validation.

## Entry Points

Канонический API:

```ts
import { registerDefinition } from '@query-graph/core'
import { defineGraph, relation, project } from '@query-graph/core/dsl'
```

<code>@query-graph/core/definition</code> сохраняет позиционные фабрики только
для обратной совместимости с ранним API 1.x.
<code>@query-graph/dsl-object</code> остается тонким deprecated facade над
<code>@query-graph/core/dsl</code>. Новому коду эти entrypoint не нужны.

Batch-фабрики временно доступны из package root, но новый код должен импортировать
их из <code>@query-graph/core/dsl</code>.

## Архитектура

<code>query-graph-core</code> не зависит от Node.js. В нем находятся wire model,
analysis, type checker, planner, relational mapping и SQL compilers. Корневой
Rust crate содержит только N-API adapter и перевод diagnostics/metadata в
JavaScript.

SQL execution намеренно остается у потребителя. Это позволяет использовать
любой pool, transaction model, tracing и политику retry, не связывая planner с
конкретным DB driver.
