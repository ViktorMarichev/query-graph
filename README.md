# query-graph

Нативное планирование графов запросов и компиляция SQL для Node.js.

Пакет отвечает за валидацию графа, планирование и генерацию SQL. Подключение к
базе данных и исполнение запросов остаются ответственностью потребителя.
Один и тот же граф и реляционное отображение могут компилироваться в SQL Server
или Oracle SQL.

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
      through: [valueRelation],
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

Полученное определение не содержит SQL, имен таблиц, значений конкретного
драйвера, объектов построителя или исполняемых функций обратного вызова
JavaScript. DSL не выполняет планирование запросов: Rust остается единственным
источником правил валидации и компиляции.

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
логическими параметрами, например `idOwner`, и содержит их scalar type и
cardinality. Потребитель берет значения из `operation.parameters` и передает их
своему драйверу базы данных.

Компилятор намеренно не использует логические имена графа как SQL-алиасы.
Источники получают короткие физические алиасы `t0`, `t1`, а выходные колонки -
`c0`, `c1`. Поэтому длинные имена источников и пути проекции не зависят от
ограничений конкретной СУБД на длину идентификаторов.

`statement.columns` сопоставляет физические имена колонок с логическими путями
проекции и цепочками relations. `statement.relations` описывает выбранный
планировщиком путь связей, включая направление, cardinality и обязательность.
Эти metadata позволяют потребителю собрать вложенный результат, не разбирая SQL.

## Oracle

Oracle использует тот же planner и operation. Отличается только финальный
dialect renderer:

```ts
const statement = relationalGraph.compileOracle(operation)

console.log(statement.sql)
console.table(statement.bindings)
```

Компилятор генерирует именованные параметры `:p0`, Oracle-кавычки идентификаторов,
нативный порядок `NULLS FIRST`/`NULLS LAST`, конкатенацию через `||` и пагинацию
`OFFSET`/`FETCH`, доступную начиная с Oracle 12c. `catalog` в `TableName` относится
к SQL Server и отклоняется Oracle-компилятором; имя схемы поддерживается.

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
