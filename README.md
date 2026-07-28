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
- сортировку по умолчанию.

`registerDefinition` один раз передает определение в Rust. Rust валидирует все
ссылки, создает переиспользуемые индексы и возвращает нативный дескриптор
`QueryGraph`.

```ts
import { registerDefinition } from 'query-graph'
import {
  asc,
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  exists,
  firstBy,
  inParameter,
  isNotNull,
  nullable,
  param,
  project,
  relation,
  requiredListParameter,
  requiredParameter,
  source,
} from 'query-graph/definition'

const link = source('link', {
  idOwner: 'int64',
  idControllerObjectValue: 'int64',
})

const value = source('value', {
  id: 'int64',
  value: nullable('string'),
})

const idOwner = requiredParameter('idOwner', 'int64')
const valueRelation = relation('value', link, value, eq(link.field('idControllerObjectValue'), value.field('id')), {
  required: true,
})

const attributeValuesModule = defineGraphModule({
  name: 'attributeValues',
  sources: [link, value],
  parameters: [idOwner],
  relations: [valueRelation],
  constraints: [
    constraint('owner', eq(link.field('idOwner'), param(idOwner))),
    constraint('hasValue', exists(value, isNotNull(value.field('value')))),
  ],
  projection: [
    project('value.value', value.field('value'), {
      default: true,
    }),
  ],
})

const definition = defineGraph({
  name: 'attributeValues',
  root: link,
  modules: [attributeValuesModule],
})

const graph = registerDefinition(definition)
```

`GraphModule` группирует переиспользуемую часть определения: источники,
параметры, связи, ограничения, проекцию и сортировку. У модуля нет собственного
корневого источника, и его нельзя зарегистрировать или скомпилировать отдельно.

`defineGraph` объединяет вложенные модули и локальные элементы в один плоский
`GraphDefinition`. Повторно использованные объекты дедуплицируются, а разные
определения с одинаковым именем приводят к ошибке композиции. Сведения о модулях
не передаются в Rust и не входят в wire-формат.

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

Canonical `GraphDefinition` не зависит от authoring DSL. Основной пакет сохраняет
functional API в `query-graph/definition`, а альтернативные DSL можно устанавливать
отдельно, не добавляя новый planner или SQL compiler.

Объектный вариант поставляется отдельным workspace/npm-пакетом:

```bash
npm install query-graph @query-graph/dsl-object
```

```ts
import { registerDefinition } from 'query-graph'
import { asc, defineGraph, eq, firstBy, project, relation, source } from '@query-graph/dsl-object'

const staff = source('staff', { id: 'int64' })
const personStaff = source('personStaff', {
  id: 'int64',
  idStaff: 'int64',
  idPerson: 'int64',
})

const definition = defineGraph({
  name: 'staff',
  root: staff,
  sources: [staff, personStaff],
  relations: [
    relation({
      name: 'credentials',
      from: staff,
      to: personStaff,
      on: eq(staff.field('id'), personStaff.field('idStaff')),
      cardinality: 'one',
      selection: firstBy(asc(personStaff.field('idPerson')), asc(personStaff.field('id'))),
    }),
  ],
  projection: [
    project({ path: 'id', expression: staff.field('id'), default: true }),
    project({ path: 'credentials.idPerson', expression: personStaff.field('idPerson') }),
  ],
})

const graph = registerDefinition(definition)
```

Оба DSL формируют один и тот же versioned wire-контракт. Сторонний DSL также может
создавать `GraphDefinitionInput`; Rust проверяет его при `registerDefinition`.

### Existential constraints

`exists(source, predicate?)` задаёт семантический квантор внутри constraint.
Указывать SQL-подзапрос или correlation columns не требуется: Rust использует
уникальный relation path от root до указанного source.

```ts
constraint('hasService', exists(businessServiceStaff, eq(businessServiceStaff.field('idService'), param(idService))))
```

Predicate может обращаться к root и sources на выведенном пути. Ссылки на
соседнюю ветку отклоняются при регистрации definition. `exists` вне constraints
также отклоняется; отрицание выражается обычным `not(exists(...))`.

Planner воспринимает existential branch как semijoin. Sources этой ветки не
добавляются во внешние JOIN и `statement.relations`, поэтому relation с
cardinality `many` не размножает корневые строки и не мешает их пагинации.
Oracle и SQL Server compiler строят коррелированный `EXISTS` из одной и той же
семантики definition.

### Параметры-списки

Параметр имеет явную форму `scalar` или `list`. Один параметр не меняет форму
между операциями, поэтому в прикладном коде не требуется поддерживать
`number | number[]`.

```ts
const idServices = requiredListParameter('idServices', 'int64')

const definition = defineGraph({
  name: 'staffByServices',
  root: staff,
  sources: [staff, businessServiceStaff],
  parameters: [idServices],
  relations: [
    relation(
      'staffServices',
      staff,
      businessServiceStaff,
      eq(staff.field('id'), businessServiceStaff.field('idStaff')),
      { cardinality: 'many' },
    ),
  ],
  constraints: [
    constraint(
      'services',
      exists(businessServiceStaff, inParameter(businessServiceStaff.field('idService'), idServices)),
    ),
  ],
  projection: [project('id', staff.field('id'), { default: true })],
})
```

Operation передает обычный массив:

```ts
const statement = relationalGraph.compileSqlServer({
  parameters: { idServices: [12, 18, 24] },
})
```

Compiler разворачивает его в `IN (@p0, @p1, @p2)`. Каждый элемент получает
binding `{ parameter: 'idServices', index, scalarType: 'int64' }`, по которому
adapter исполнения берет значение из исходного массива. Rust проверяет каждый
элемент отдельно и возвращает путь ошибки наподобие
`parameters.idServices[2]`.

Отсутствующий optional list отключает constraint с `when`. Переданный пустой
список остается присутствующим параметром и компилируется в ложный предикат
`1 = 0`; недопустимый SQL `IN ()` не создается.

### To-one selection

`cardinality: 'one'` описывает контракт связи. Если физический источник может
содержать несколько кандидатов, `firstBy` задает семантическое правило выбора
одной строки:

```ts
const credentials = relation('credentials', staff, personStaff, eq(staff.field('id'), personStaff.field('idStaff')), {
  selection: firstBy(asc(personStaff.field('idPerson')), asc(personStaff.field('id'))),
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
const serviceId = dimension('serviceId', service.field('id'), {
  default: true,
})

const staffCount = measure('staffCount', countDistinct(serviceStaff.field('idStaff')), { default: true })

const definition = defineSummaryGraph({
  name: 'serviceSummary',
  root: service,
  sources: [service, serviceStaff],
  parameters: [idOrganisation, minimumStaff],
  relations: [
    relation('staff', service, serviceStaff, eq(service.field('id'), serviceStaff.field('idService')), {
      cardinality: 'many',
    }),
  ],
  constraints: [
    constraint('organisation', eq(service.field('idOrganisation'), param(idOrganisation))),
    constraint('minimumStaff', gte(staffCount, param(minimumStaff))),
  ],
  dimensions: [serviceId],
  measures: [staffCount],
  defaultOrderBy: [desc(staffCount)],
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
  select: ['id', 'credentials.idPerson'],
  parameters: {
    idOrganisation: 42,
    personIds: [7, 11],
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

## Диагностика

Ошибки N-API имеют имя `QueryGraphError` и сохраняют обычный `message` и stack.
Для программной обработки доступны:

- `code` — категория ошибки на границе пакета;
- `phase` — `definition`, `mapping`, `operation` или `sql`;
- `issues` — массив `{ code, location, message }`.

```ts
import type { QueryGraphError } from 'query-graph/definition'

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
    link: {
      table: {
        schema: 'dbo',
        name: 'ControllerAttributeValueLink',
      },
      columns: {
        idOwner: 'owner_id',
      },
    },
    value: {
      table: 'ControllerObjectValue',
    },
  },
})
```

Операция передается в Rust одним вызовом. Компилятор выбирает необходимые пути
связей, формирует синтаксис SQL Server и возвращает описания параметров.

```ts
const operation = {
  select: ['value.value'],
  parameters: {
    idOwner: 42,
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
логическими параметрами, например `idOwner`, и содержит их scalar type.
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
