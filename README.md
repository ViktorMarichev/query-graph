# query-graph

Нативное планирование графов запросов и компиляция SQL для Node.js.

Пакет отвечает за валидацию графа, планирование и генерацию SQL. Подключение к
базе данных и исполнение запросов остаются ответственностью потребителя.

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
```

`statement.bindings` связывает сгенерированные имена, например `p0`, с
логическими параметрами, например `idOwner`. Потребитель берет значения из
`operation.parameters` и передает их своему драйверу базы данных.

Текущая реализация SQL Server поддерживает выбор полей проекции, ограничения
определения, условные ограничения, сортировку по умолчанию, соединения
`INNER JOIN`/`LEFT JOIN` и пагинацию `OFFSET`/`FETCH`. Фильтры времени выполнения
и пользовательские отображения семантических функций намеренно оставлены для
последующих этапов.

Полный пример графа, отображения, операции, сгенерированного SQL и замера
производительности компиляции находится в `benchmark/bench.ts`.
