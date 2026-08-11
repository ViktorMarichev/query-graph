# query-graph

Нативное планирование графов запросов и компиляция SQL для Node.js.

Пакет отвечает за валидацию графа, планирование и генерацию SQL. Подключение к
базе данных и исполнение запросов остаются ответственностью потребителя.
Один и тот же граф и реляционное отображение могут компилироваться в SQL Server
или Oracle SQL.

## Архитектура пакета

Cargo workspace разделён на два слоя. `query-graph-core` не зависит от Node.js
или N-API и содержит definition, type system, planner, relational mapping и SQL
compilers. Корневой crate `query_graph` является тонким N-API adapter: он
десериализует wire-объекты, вызывает core и переводит результат и diagnostics в
JavaScript.

Зависимость направлена только от adapter к core. Исполнение SQL в оба слоя не входит.

## Определение графа

Определение графа не зависит от диалекта SQL и содержит:

- логические источники и поля;
- типизированные параметры;
- именованные связи;
- семантические ограничения;
- выходную проекцию;
- именованные варианты сортировки.

`registerDefinition` один раз передает определение в Rust. Rust валидирует все
ссылки, создает переиспользуемые индексы и возвращает нативный дескриптор
`QueryGraph`.

```ts
import { registerDefinition } from '@query-graph/core'
import {
  asc,
  constraint,
  desc,
  defineGraph,
  defineGraphModule,
  eq,
  exists,
  firstBy,
  inParameter,
  isNotNull,
  nullable,
  ordering,
  param,
  project,
  relation,
  requiredListParameter,
  requiredParameter,
  source,
} from '@query-graph/core/dsl'

const users = source('users', {
  id: 'int64',
  email: 'string',
})

const profiles = source('profiles', {
  id: 'int64',
  userId: 'int64',
  displayName: nullable('string'),
})

const userId = requiredParameter('userId', 'int64')
const profileRelation = relation({
  name: 'profile',
  from: users,
  to: profiles,
  on: eq(users.field('id'), profiles.field('userId')),
  cardinality: 'one',
  required: true,
  selection: firstBy(asc(profiles.field('id'))),
})

const userProfileModule = defineGraphModule({
  name: 'userProfile',
  sources: [users, profiles],
  parameters: [userId],
  relations: [profileRelation],
  constraints: [
    constraint({
      predicate: eq(users.field('id'), param(userId)),
    }),
    constraint({
      predicate: exists(profiles, isNotNull(profiles.field('displayName'))),
    }),
  ],
  projection: [
    project({
      path: 'id',
      expression: users.field('id'),
      default: true,
    }),
    project({
      path: 'profile.displayName',
      expression: profiles.field('displayName'),
    }),
  ],
  orderings: [
    ordering({
      name: 'idAsc',
      by: [asc(users.field('id'))],
      default: true,
    }),
  ],
})

const definition = defineGraph({
  name: 'userProfile',
  root: users,
  modules: [userProfileModule],
})

const graph = registerDefinition(definition)
```

`GraphModule` группирует переиспользуемую часть определения: источники,
параметры, связи, ограничения, проекцию и сортировку. У модуля нет собственного
корневого источника, и его нельзя зарегистрировать или скомпилировать отдельно.

`defineGraph` объединяет вложенные модули и локальные элементы в один плоский
`GraphDefinition`. Повторно подключенный объект дедуплицируется по identity. Для
именованных элементов разные определения с одним ключом приводят к ошибке
композиции.

Constraints не имеют имени: повторно использованный constraint добавляется один
раз, а разные constraint-объекты сохраняются и объединяются через `AND`. Сведения
о модулях не передаются в Rust и не входят в wire-формат.

Вспомогательные функции API описания графа формируют версионируемое и
сериализуемое промежуточное представление `GraphDefinition`. Дискриминаторы
выражений, например `kind: 'eq'`, существуют только в сгенерированном
представлении. В прикладном коде графа их писать не требуется. TypeScript
проверяет ссылки на поля источников.

Relation path проекции указывать не требуется. При регистрации definition Rust
находит самый глубокий source, использованный expression, и один раз сохраняет
путь от root до этого source. Expression может обращаться к root и любым sources
на одной ветке. Обращение к двум независимым веткам отклоняется как неоднозначная
проекция. Planner использует уже вычисленные пути и не повторяет inference при
каждой компиляции operation.

### Варианты DSL

Canonical `GraphDefinition` не зависит от authoring DSL. Основной authoring API
доступен через `@query-graph/core/dsl` и использует объектные конфигурации. Позиционный
`@query-graph/core/definition` сохранён как адаптер совместимости для существующего кода;
новые графы на него ориентировать не следует.

Отдельный пакет `@query-graph/dsl-object` является тонким фасадом над тем же API.
Он полезен, когда DSL нужен как отдельная зависимость:

```bash
npm install @query-graph/core @query-graph/dsl-object
```

```ts
import { registerDefinition } from '@query-graph/core'
import { asc, defineGraph, eq, firstBy, project, relation, source } from '@query-graph/dsl-object'

const users = source('users', { id: 'int64' })
const profiles = source('profiles', {
  id: 'int64',
  userId: 'int64',
  displayName: 'string',
})

const definition = defineGraph({
  name: 'users',
  root: users,
  sources: [users, profiles],
  relations: [
    relation({
      name: 'profile',
      from: users,
      to: profiles,
      on: eq(users.field('id'), profiles.field('userId')),
      cardinality: 'one',
      selection: firstBy(asc(profiles.field('id'))),
    }),
  ],
  projection: [
    project({ path: 'id', expression: users.field('id'), default: true }),
    project({ path: 'profile.displayName', expression: profiles.field('displayName') }),
  ],
})

const graph = registerDefinition(definition)
```

Фасад не содержит второй реализации и напрямую переэкспортирует
`@query-graph/core/dsl`. Расширение позиционного compatibility API не меняет поверхность
объектного DSL.

Оба входа формируют один и тот же versioned wire-контракт. Сторонний DSL также может
создавать `GraphDefinitionInput`; Rust проверяет его при `registerDefinition`.

### Existential constraints

`exists(source, predicate?)` задаёт семантический квантор внутри constraint.
Указывать SQL-подзапрос или correlation columns не требуется: Rust использует
уникальный relation path от root до указанного source.

```ts
constraint({
  predicate: exists(memberships, eq(memberships.field('teamId'), param(teamId))),
})
```

Predicate может обращаться к root и sources на выведенном пути. Ссылки на
соседнюю ветку отклоняются при регистрации definition. `exists` вне constraints
также отклоняется; отрицание выражается обычным `not(exists(...))`.

Planner воспринимает existential branch как semijoin. Sources этой ветки не
добавляются во внешние JOIN и `statement.relations`, поэтому relation с
cardinality `many` не размножает корневые строки и не мешает их пагинации.
Oracle и SQL Server compiler строят коррелированный `EXISTS` из одной и той же
семантики definition.

По умолчанию `exists` коррелируется от root. Для semijoin относительно уже
выбранного внешнего source можно явно указать точку корреляции:

```ts
constraint({
  predicate: exists(permissions, permissionPredicate, { from: memberships }),
})
```

`from` должен быть строгим предком целевого source на relation path. Planner
оставляет путь до `from` во внешнем запросе, а оставшийся путь компилирует как
коррелированный semijoin.

### Параметры-списки

Параметр имеет явную форму `scalar` или `list`. Один параметр не меняет форму
между операциями, поэтому в прикладном коде не требуется поддерживать
`number | number[]`.

```ts
const users = source('users', {
  id: 'int64',
})

const memberships = source('memberships', {
  id: 'int64',
  userId: 'int64',
  teamId: 'int64',
})

const teamIds = requiredListParameter('teamIds', 'int64')

const definition = defineGraph({
  name: 'usersByTeams',
  root: users,
  sources: [users, memberships],
  parameters: [teamIds],
  relations: [
    relation({
      name: 'memberships',
      from: users,
      to: memberships,
      on: eq(users.field('id'), memberships.field('userId')),
      cardinality: 'many',
    }),
  ],
  constraints: [
    constraint({
      predicate: exists(memberships, inParameter(memberships.field('teamId'), teamIds)),
    }),
  ],
  projection: [project({ path: 'id', expression: users.field('id'), default: true })],
})
```

Operation передает обычный массив:

```ts
const statement = relationalGraph.compileSqlServer({
  parameters: { teamIds: [12, 18, 24] },
})
```

Compiler разворачивает его в `IN (@p0, @p1, @p2)`. Каждый элемент получает
binding `{ parameter: 'teamIds', index, scalarType: 'int64' }`, по которому
adapter исполнения берет значение из исходного массива. Rust проверяет каждый
элемент отдельно и возвращает путь ошибки наподобие
`parameters.teamIds[2]`.

Отсутствующий optional list отключает constraint с `when`. Переданный пустой
список остается присутствующим параметром и компилируется в ложный предикат
`1 = 0`; недопустимый SQL `IN ()` не создается.

### Именованные сортировки

Definition объявляет допустимые способы упорядочивания, а operation выбирает один из них по имени:

```ts
const definition = defineGraph({
  // ...
  orderings: [
    ordering({ name: 'createdDesc', by: [desc(items.field('dateCreate'))], default: true }),
    ordering({ name: 'nameAsc', by: [asc(items.field('name'))] }),
  ],
})

relationalGraph.compileOracle({
  ordering: 'nameAsc',
  parameters,
})
```

Поле `default: true` можно указать только у одного варианта. Если operation не передает
`ordering`, planner использует default-вариант; без него SQL строится без `ORDER BY`.
TypeScript выводит union имен из definition и подключенных graph modules. Rust проверяет имя
повторно и планирует только связи и параметры выбранной сортировки.

### To-one selection

`cardinality: 'one'` описывает контракт связи. Если физический источник может
содержать несколько кандидатов, `firstBy` задает семантическое правило выбора
одной строки:

```ts
const profile = relation({
  name: 'profile',
  from: users,
  to: profiles,
  on: eq(users.field('id'), profiles.field('userId')),
  selection: firstBy(asc(profiles.field('id'))),
})
```

Поля сортировки могут обращаться только к target source. Список сортировки
обязателен; последнее поле следует выбирать уникальным, чтобы результат был
детерминированным. `required: false` означает `0..1`, а `required: true` -
ровно одну связанную строку.

SQL Server renderer использует `OUTER/CROSS APPLY` и `TOP (1)`, Oracle 12c+
использует `OUTER/CROSS APPLY` и `FETCH FIRST 1 ROW ONLY`. Эти детали не входят
в definition. Для Oracle 11g `firstBy` возвращает
`unsupportedDialectFeature`.

### Summary-графы

`defineSummaryGraph` описывает результат через dimensions и measures, не через
SQL-конструкции `GROUP BY` и `HAVING`:

```ts
const teams = source('teams', {
  id: 'int64',
  workspaceId: 'int64',
})

const memberships = source('memberships', {
  id: 'int64',
  userId: 'int64',
  teamId: 'int64',
})

const workspaceId = requiredParameter('workspaceId', 'int64')
const minimumMembers = requiredParameter('minimumMembers', 'int64')

const teamId = dimension({
  path: 'teamId',
  expression: teams.field('id'),
  default: true,
})

const memberCount = measure({
  path: 'memberCount',
  expression: countDistinct(memberships.field('userId')),
  default: true,
})

const definition = defineSummaryGraph({
  name: 'teamSummary',
  root: teams,
  sources: [teams, memberships],
  parameters: [workspaceId, minimumMembers],
  relations: [
    relation({
      name: 'members',
      from: teams,
      to: memberships,
      on: eq(teams.field('id'), memberships.field('teamId')),
      cardinality: 'many',
    }),
  ],
  constraints: [
    constraint({
      predicate: eq(teams.field('workspaceId'), param(workspaceId)),
    }),
    constraint({
      predicate: gte(memberCount, param(minimumMembers)),
    }),
  ],
  dimensions: [teamId],
  measures: [memberCount],
  orderings: [
    ordering({
      name: 'memberCountDesc',
      by: [desc(memberCount)],
      default: true,
    }),
  ],
})
```

Planner включает все объявленные dimensions в идентичность результата и
компилирует их в `GROUP BY`, даже если operation не возвращает часть из них.
Constraint без агрегата фильтрует исходное множество через `WHERE`. Constraint
с агрегатом фильтрует уже построенные группы через `HAVING`. Поэтому автор графа
не выбирает SQL-фазу вручную и не может случайно поместить агрегат в `WHERE`.

Поддерживаются `count()`, `count(expression)`, `countDistinct`, `sum`,
`average`, `minimum` и `maximum`. Measure можно передавать в сравнения и
сортировку как expression; DSL разворачивает ссылку в сериализуемое
семантическое выражение до передачи definition в Rust.

Rust отклоняет агрегаты в обычных graph projections, relation predicates и
dimensions, а также вложенные агрегаты и поля вне объявленных dimensions.
`count` и `countDistinct` возвращают non-null `int64`; остальные меры nullable,
поскольку пустое множество или множество из `NULL` не имеет значения агрегата.
Для сохранения контракта `int64` SQL Server compiler использует `COUNT_BIG`, а
Oracle compiler - `COUNT`. Dimension должен иметь семантику равенства, поэтому
`json` не может задавать идентичность группы.

Глобальный `DISTINCT` намеренно не выводится из структуры связей. Для количества
уникальных значений используется `countDistinct`, а summary только из dimensions
задает уникальные комбинации через группировку. Произвольный SQL subquery также
не входит в DSL: `exists`, `firstBy` и агрегаты остаются семантическими
операциями, для которых форму подзапроса при необходимости выбирает compiler.

Summary-план с несколькими независимыми relations `many` отклоняется: обычные
JOIN создали бы декартово размножение веток и могли исказить measures. Ограничение
соседней ветки следует выражать через `exists`. Независимые меры нескольких
коллекций потребуют отдельной aggregate-subquery strategy в planner; compiler не
маскирует эту ситуацию через `DISTINCT`.

Полученное определение не содержит SQL, имен таблиц, значений конкретного
драйвера, объектов построителя или исполняемых функций обратного вызова
JavaScript. DSL не выполняет планирование запросов: Rust остается единственным
источником правил валидации и компиляции.

## Система типов

При `registerDefinition` Rust выводит тип каждого expression из описаний полей,
параметров и литералов. Проверка выполняется один раз после структурной
валидации и до построения переиспользуемого `QueryGraph`.

Система типов проверяет:

- совместимость операндов сравнений;
- строковые аргументы `like`, `lower`, `upper` и `concat`;
- общий тип аргументов `coalesce`;
- boolean-тип условий relations и constraints;
- допустимость сортировки выражений;
- конкретный scalar type и nullability каждого поля проекции;
- типы аргументов агрегатов и их результирующую nullability;
- соответствие неагрегированных выражений объявленным dimensions.

Поддерживаемые семантические функции задаются DSL-функциями `lower`, `upper`,
`coalesce` и `concat`. Произвольное имя функции не является частью публичного
DSL: сначала функция должна получить общую семантику типов и реализацию для
каждого SQL dialect.

TypeScript проверяет имена источников, полей и семантических функций во время
написания definition. Rust остается авторитетной проверкой scalar-типов после
композиции модулей; Node.js не обходит граф и не дублирует правила вывода типов.

`defineGraph` также сохраняет типы параметров и projection paths, включая
содержимое вложенных `GraphModule`. `registerDefinition` переносит эти сведения
на native handle, поэтому операция проверяется непосредственно в месте вызова:

```ts
const relationalGraph = registerDefinition(definition).withRelationalMapping(mapping)

relationalGraph.compileSqlServer({
  select: ['id', 'profile.displayName'],
  parameters: {
    userId: 42,
  },
})
```

TypeScript отклоняет неизвестный projection path или параметр, отсутствие
required-параметра, scalar вместо списка и значение неподходящего scalar type.
Optional-параметры можно не передавать, если они не требуются выбранным планом;
обычно они включают constraint через `when`. Для `int64` допустимы безопасный
JavaScript number и десятичная строка, а list-параметр получает массив того же
типа. Эти проверки существуют только в декларациях TypeScript; сериализация
definition, N-API вызов, runtime-валидация Rust и SQL compilation не меняются.

### Тип результата проекции

DSL сохраняет scalar type и nullability каждого projection expression в compile-time metadata.
`ResultOf` строит из этих сведений вложенный тип результата по projection paths:

```ts
import type { QueryOperation, ResultOf } from '@query-graph/core/dsl'

const operation = {
  select: ['id', 'profile.displayName'],
  parameters: { userId: 42 },
} as const satisfies QueryOperation<typeof definition>

type UserRow = ResultOf<typeof definition, typeof operation>
// {
//   id: number | string
//   profile: { displayName: string | null }
// }
```

Если второй аргумент не передан, в результат входят только поля с `default: true`.
`ResultOf` также принимает тип, возвращенный `registerDefinition`, или mapped graph:

```ts
type DefaultUserRow = ResultOf<typeof graph>
// { id: number | string }
```

Nullability учитывает nullable-поле и всю цепочку optional relations до его source.
Required relation после optional relation не делает значение обязательным. Семантика
выражений тоже сохраняется: например, `coalesce(nullableField, 'fallback')` выводится
как non-null `string`.

Типы значений по умолчанию нейтральны к DB driver: `int64` и `decimal` представлены
как `number | string`, а `date`, `dateTime` и `binary` - как `string`. Адаптер
исполнения может передать третьим аргументом собственную полную scalar map:

```ts
import type { DefaultScalarOutputTypeMap, ResultOf } from '@query-graph/core/dsl'

type DriverScalars = Omit<DefaultScalarOutputTypeMap, 'int64' | 'dateTime'> & {
  int64: bigint
  dateTime: Date
}

type DriverUserRow = ResultOf<typeof definition, typeof operation, DriverScalars>
```

Для точного учета явного `select` operation должна сохранять literal tuple через
`as const` или `satisfies`. Это только TypeScript API: wire definition, N-API и
Rust planner не получают дополнительного runtime state.

`ResultOf` описывает логическую форму одной строки, восстановленную из путей
проекции. Он не выполняет запрос и не превращает relation `many` в массив.
Группировка строк, дедупликация и гидратация коллекций остаются ответственностью
потребителя модуля.

## Диагностика

Ошибки N-API имеют имя `QueryGraphError` и сохраняют обычный `message` и stack.
Для программной обработки доступны:

- `code` — категория ошибки на границе пакета;
- `phase` — `definition`, `mapping`, `operation` или `sql`;
- `issues` — массив `{ code, location, message }`.

```ts
import type { QueryGraphError } from '@query-graph/core/definition'

try {
  relationalGraph.compileSqlServer(operation)
} catch (cause) {
  const error = cause as QueryGraphError

  if (error.name === 'QueryGraphError') {
    console.table(error.issues)
  }
}
```

Wire errors, semantic validation и SQL compilation имеют разные верхнеуровневые
`code`. Коды отдельных `issues` стабильны и не требуют разбора текста `message`.

## SQL Server

Реляционное отображение связывает логические источники с физическими таблицами.
По умолчанию логические имена полей используются как имена колонок. В `columns`
указываются только переопределения.

```ts
const relationalGraph = graph.withRelationalMapping({
  sources: {
    users: {
      table: {
        schema: 'dbo',
        name: 'users',
      },
    },
    profiles: {
      table: {
        schema: 'dbo',
        name: 'user_profiles',
      },
      columns: {
        userId: 'user_id',
        displayName: 'display_name',
      },
    },
  },
})
```

Операция передается в Rust одним вызовом. Компилятор выбирает необходимые пути
связей, формирует синтаксис SQL Server и возвращает описания параметров.

```ts
const operation = {
  select: ['id', 'profile.displayName'],
  parameters: {
    userId: 42,
  },
  offset: 0,
  limit: 25,
}

const statement = relationalGraph.compileSqlServer(operation)

console.log(statement.sql)
console.table(statement.bindings)
console.table(statement.columns)
console.table(statement.relations)
```

`statement.bindings` связывает сгенерированные имена, например `p0`, с
логическими параметрами, например `userId`, и содержит их scalar type.
Потребитель берет значения из `operation.parameters` и передает их
своему драйверу базы данных.

Wire-контракт значений параметров:

| Scalar type | Значение в `operation.parameters`                              |
| ----------- | -------------------------------------------------------------- |
| `boolean`   | `boolean`                                                      |
| `int32`     | целое JavaScript number в диапазоне `int32`                    |
| `int64`     | безопасное целое JavaScript number или десятичная строка `i64` |
| `float64`   | JSON number                                                    |
| `decimal`   | JSON number или десятичная строка без экспоненты               |
| `string`    | string                                                         |
| `date`      | непрозрачная для compiler строка                               |
| `dateTime`  | непрозрачная для compiler строка                               |
| `binary`    | непрозрачная для compiler строка                               |
| `json`      | любое JSON-совместимое значение                                |

Optional parameter означает отсутствие ключа, а не `null`. Формат строк
`date`, `dateTime` и `binary`, а также преобразование scalar type в тип конкретного
DB driver определяет adapter исполнения. SQL compiler не копирует значения в
`statement`: binding metadata ссылается на исходный ключ параметра.

Целевую версию можно указать отдельно от operation:

```ts
const statement = relationalGraph.compileSqlServer(operation, { version: '2019' })
```

По умолчанию используется SQL Server 2012. SQL Server 2008 поддерживается для
запросов без `OFFSET/FETCH`; несовместимая pagination отклоняется до исполнения.

Компилятор намеренно не использует логические имена графа как SQL-алиасы.
Источники получают короткие физические алиасы `t0`, `t1`, а выходные колонки -
`c0`, `c1`. Поэтому длинные имена источников и пути проекции не зависят от
ограничений конкретной СУБД на длину идентификаторов.

`statement.columns` сопоставляет физические имена колонок с логическими путями
проекции, содержит их `scalarType`, `nullable` и цепочки relations.
`statement.relations` описывает выбранный планировщиком путь связей, включая
направление, cardinality и фактическую обязательность JOIN. Эти metadata позволяют
потребителю сопоставить плоские SQL-колонки с логическими путями, не разбирая SQL.
Группировка строк и гидратация коллекций не входят в контракт пакета и остаются
ответственностью потребителя.

## Oracle

Oracle использует тот же planner и operation. Отличается только финальный
dialect renderer:

```ts
const statement = relationalGraph.compileOracle(operation)
const oracle19cStatement = relationalGraph.compileOracle(operation, { version: '19c' })

console.log(statement.sql)
console.table(statement.bindings)
```

Компилятор генерирует именованные параметры `:p0`, Oracle-кавычки идентификаторов,
нативный порядок `NULLS FIRST`/`NULLS LAST`, конкатенацию через `||` и пагинацию
`OFFSET`/`FETCH`, доступную начиная с Oracle 12c. `catalog` в `TableName` относится
к SQL Server и отклоняется Oracle-компилятором; имя схемы поддерживается.

По умолчанию выбирается Oracle 12c. Oracle 11g поддерживается для запросов без
pagination; `OFFSET/FETCH` для него отклоняется как неподдерживаемая capability.

Оба компилятора поддерживают выбор полей проекции, scalar/list параметры,
ограничения определения,
условные ограничения, сортировку по умолчанию, соединения
`INNER JOIN`/`LEFT JOIN`, summary dimensions/measures и пагинацию
`OFFSET`/`FETCH`. Общий SQL pipeline выбирает пути связей, фазы `WHERE`/`HAVING`
и обходит expression AST, а dialect renderer отвечает только за синтаксис
конкретной СУБД.

Для обычного record-графа пагинация отклоняется, если план проходит через
relation с cardinality `many`: JOIN в таком случае меняет количество корневых
строк. Summary-граф может пагинировать результат после группировки, поэтому
relation `many` для него допустим. Для пагинации вложенных record-коллекций
по-прежнему понадобится отдельный split-query plan.

Фильтры времени выполнения и пользовательские отображения семантических функций
намеренно оставлены для последующих этапов.

Полный пример графа, отображения, операции, сгенерированного SQL и замера
производительности компиляции находится в `benchmark/bench.ts`.

## Batch-связи и двухфазное выполнение

Поля из отдельного запроса подключаются без изменения обычного relational-графа:

```ts
const childrenQuery = batchQuery({
  graph: childrenGraph,
  key: {
    path: 'parentId',
    parameter: parentIds,
  },
})

const children = batchRelation({
  name: 'children',
  from: 'id',
  query: childrenQuery,
  cardinality: 'many',
})

const graph = composeGraph({ root: rootGraph, relations: [children] })
const plan = graph.compileOraclePlan({
  select: ['id', 'children.id'],
  parameters: {},
  limit: 20,
})
```

`batchQuery` принадлежит child-модулю и один раз закрепляет его graph,
projection path ключа и list-параметр. Поэтому разные root-графы могут
переиспользовать query, не повторяя внутренний контракт child-запроса.

`composeGraph` оставляет в JavaScript только декларативный фасад. Проверка
совместимости ключей и параметров, разделение operation на root и batch-шаги и
компиляция SQL выполняются в Rust. `compileOraclePlan` или
`compileSqlServerPlan` сразу компилирует только `plan.root`; child SQL создаётся
при вызове `compileBatch`. Plan сохраняет выбранные dialect и version, поэтому
отложенный statement компилируется с теми же настройками, что и root.

Для подготовки child bindings каждый шаг в `plan.batches` также содержит имя
ключевого параметра `keyParameter` и снимок статических `parameters`. Executor
объединяет их с `{ [batch.keyParameter]: keys }` и сопоставляет полученные значения
с возвращёнными `statement.bindings`.

Executor потребителя выполняет `plan.root`, собирает уникальные ненулевые
значения `parentKey` из `plan.batches`, вызывает
`plan.compileBatch('children', parentIds)`, выполняет child statement и
связывает строки по `childKey`. `cardinality: 'one'` означает объект или `null`,
а `many` — массив. Query-graph не подключается к БД, не дедуплицирует ключи, не
выполняет statements и не гидратирует результат; при пустом наборе ключей
executor должен пропустить child-запрос.

Ordering, offset и limit корневой operation применяются только к `plan.root`.
Batch-связь может задать собственный `ordering`, но child pagination не
добавляется. Batch-поля выбираются только явно (`children.id`); без prefixed
path соответствующего шага в `plan.batches` нет. Флаги injection в metadata
показывают executor, какие ключи были добавлены исключительно для связывания и
не должны попасть в публичный результат.
