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
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  nullable,
  param,
  project,
  relation,
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
  constraints: [constraint('owner', eq(link.field('idOwner'), param(idOwner)))],
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
- конкретный scalar type и nullability каждого поля проекции.

Поддерживаемые семантические функции задаются DSL-функциями `lower`, `upper`,
`coalesce` и `concat`. Произвольное имя функции не является частью публичного
DSL: сначала функция должна получить общую семантику типов и реализацию для
каждого SQL dialect.

TypeScript проверяет имена источников, полей и семантических функций во время
написания definition. Rust остается авторитетной проверкой scalar-типов после
композиции модулей; Node.js не обходит граф и не дублирует правила вывода типов.

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

Оба компилятора поддерживают выбор полей проекции, ограничения определения,
условные ограничения, сортировку по умолчанию, соединения
`INNER JOIN`/`LEFT JOIN` и пагинацию `OFFSET`/`FETCH`. Общий SQL pipeline выбирает
пути связей и обходит expression AST, а dialect renderer отвечает только за
синтаксис конкретной СУБД.

Пагинация отклоняется, если план проходит через relation с cardinality `many`:
обычный JOIN в таком случае меняет количество корневых строк. Для этого сценария
понадобится отдельный split-query plan.

Фильтры времени выполнения, параметры-массивы и пользовательские отображения
семантических функций намеренно оставлены для последующих этапов.

Полный пример графа, отображения, операции, сгенерированного SQL и замера
производительности компиляции находится в `benchmark/bench.ts`.
