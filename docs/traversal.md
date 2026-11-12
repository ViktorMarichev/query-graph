# Recursive Traversal

`reachable` проверяет, существует ли подходящая строка среди достижимых из
заданного начала. Это boolean predicate для `constraint`, а не новый executor
или способ загрузить все строки пути. В текущей реализации поддерживается
Oracle 11g и новее; SQL Server возвращает явную ошибку `unsupportedDialectFeature`.

## Пример: корни связанных объектов

У каждой строки связи есть собственный `id`, ссылка `parentId` на родительскую
связь и два конца `fromId`/`toId`. Нужно найти корневые объекты, поднимаясь от
выбранного объекта. На всём пути оба конца связи должны быть видны организации.

```ts
import { registerDefinition } from '@query-graph/core'
import {
  and,
  constraint,
  defineGraph,
  defineGraphModule,
  eq,
  exists,
  isNull,
  nullable,
  project,
  reachable,
  relation,
  requiredParam,
  source,
} from '@query-graph/core/dsl'

const roots = source('roots', { id: 'int64' })
const relations = source('ancestorRelations', {
  id: 'int64',
  parentId: nullable('int64'),
  fromId: 'int64',
  toId: 'int64',
  deletedAt: nullable('dateTime'),
})
const nodeFields = {
  id: 'int64',
  organisationId: 'int64',
  deletedAt: nullable('dateTime'),
} as const
const fromNode = source('fromNode', nodeFields)
const toNode = source('toNode', nodeFields)
const organisationId = requiredParam('organisationId', 'int64')
const nodeId = requiredParam('nodeId', 'int64')

const relationScope = defineGraphModule({
  name: 'relationScope',
  sources: [relations, fromNode, toNode],
  parameters: [organisationId],
  relations: [
    relation({ name: 'from', from: relations, to: fromNode, on: eq(relations.field('fromId'), fromNode.field('id')) }),
    relation({ name: 'to', from: relations, to: toNode, on: eq(relations.field('toId'), toNode.field('id')) }),
  ],
})

const visibleRelation = and(
  isNull(relations.field('deletedAt')),
  exists(fromNode, and(isNull(fromNode.field('deletedAt')), eq(fromNode.field('organisationId'), organisationId))),
  exists(toNode, and(isNull(toNode.field('deletedAt')), eq(toNode.field('organisationId'), organisationId))),
)

const definition = defineGraph({
  name: 'relatedRoots',
  root: roots,
  sources: [roots],
  parameters: [organisationId, nodeId],
  constraints: [
    constraint({
      predicate: reachable({
        source: relations,
        modules: [relationScope],
        key: relations.field('id'),
        where: visibleRelation,
        start: eq(relations.field('toId'), nodeId),
        step: ({ current, next }) =>
          and(eq(current.field('parentId'), next.field('id')), eq(current.field('fromId'), next.field('toId'))),
        match: and(isNull(relations.field('parentId')), eq(relations.field('fromId'), roots.field('id'))),
        cycles: 'skip',
      }),
    }),
  ],
  projection: [project({ path: 'id', expression: roots.field('id'), default: true })],
})

const graph = registerDefinition(definition).withRelationalMapping({
  sources: {
    roots: { table: 'Node' },
    ancestorRelations: { table: 'NodeRelation' },
    fromNode: { table: 'Node' },
    toNode: { table: 'Node' },
  },
})

const statement = graph.compileOracle({
  parameters: { organisationId: 7, nodeId: 20 },
})
```

`visibleRelation` здесь обычное выражение, не специальная функция библиотеки.
`source: relations` указывает на источник строк обхода, а не на объект `relation()`.
Локальные `exists` коррелируются со строкой связи через `relationScope`.
Поле `roots.id` в `match` связывает найденную строку с текущим внешним объектом.

## Этапы обхода

1. `source`, `modules` и `where` формируют допустимое множество строк.
   Constraints локальных модулей также применяются здесь, включая `when`.
2. `start` выбирает начальные строки из этого множества.
3. `step` задаёт переход от текущей строки к следующей. Следующая строка всегда
   берётся из того же отфильтрованного множества.
4. `match` проверяет достижимые строки, включая начальные. Совпадение хотя бы
   одной строки делает predicate истинным.

В Oracle форма запроса такая:

```sql
WHERE EXISTS (
  SELECT 1
  FROM (
    SELECT walk.*
    FROM (
      SELECT eligible.*
      FROM NodeRelation eligible
      WHERE /* visibility, including correlated endpoint EXISTS */
    ) walk
    START WITH /* start */
    CONNECT BY NOCYCLE /* step, with PRIOR for current */
  ) reached
  WHERE /* match, correlated with the outer row */
)
```

Это схема SQL, а не строка для исполнения. Реальный compiler экранирует имена,
применяет mapping колонок и добавляет `PRIOR key IS NOT NULL`, включая уникальный
ключ строки в набор `PRIOR`-выражений. Значения parameters остаются bindings.

Фильтры видимости нельзя переносить в `match`: тогда невидимая строка могла бы
участвовать в пути. Отсутствие доступного родителя не делает строку корнем:
в примере корень определяется явно через `parentId IS NULL`.
Порядок обработки hierarchical queries описан в
[Oracle SQL Reference](https://docs.oracle.com/en/database/oracle/oracle-database/19/sqlrf/Hierarchical-Queries.html).

## Контракт и ограничения

- `key` должен быть non-null `int32`, `int64` или `string` и уникально определять
  строку источника. Уникальность данных обеспечивает потребитель, compiler
  проверяет только схему. Составной ключ пока не поддерживается.
- `cycles: 'skip'` используется по умолчанию. Циклическое продолжение пути
  отсекается; предел глубины, способный молча потерять корни, не добавляется.
- Callback `step` выполняется один раз при authoring и создаёт AST. `current`
  и `next` не являются значениями запроса. Rust проверяет их поля и scope.
- В `step` пока разрешены `eq(current.field(...), next.field(...))` и их
  комбинации через `and`. Направление сравнения можно поменять местами.
- `where` и `start` могут ссылаться на поля локального source, parameters и
  связанные строки через локальные `exists`. Прямые ссылки на поля других
  sources здесь запрещены, чтобы JOIN не размножал строки обхода.
- `match` может использовать поля source обхода, внешние sources и parameters,
  но не `exists`, `allGroups`, aggregates или вложенный `reachable`.
- `reachable` разрешён в boolean-позициях constraints, в том числе под
  `and`, `or`, `not`. Projection, ordering, relation.on, aggregate expressions
  и вложенные обходы пока не поддерживаются.
- Локальные модули содержат sources, relations, parameters и constraints,
  но не projection, objects или ordering. Их source keys должны отличаться
  от внешних. Между отдельными обходами одинаковый key допустим только для
  одинаковых определений полей.
- Parameters объявляются на внешнем графе. Если локальный модуль объявляет
  parameter, его определение должно совпадать с внешним. Это не создаёт
  отдельный объект operation parameters или отдельный SQL-запрос.
- Mapping включает внешние и локальные sources. Несколько логических sources
  могут ссылаться на одну физическую таблицу.

Граф регистрируется один раз. Локальные topology и типы проверяются при
регистрации; bindings и SQL строятся независимо для каждой operation.
Внешние projection, ordering, pagination и форма результата не меняются.
Рекурсивные строки не включаются в result metadata: `reachable` только фильтрует
внешние строки и не размножает их при нескольких путях к одному объекту.

Библиотека не исполняет запрос и не контролирует время работы СУБД. Даже с
защитой от циклов разветвлённый граф может иметь много путей; индексы, timeout
и проверка плана на реальных данных остаются ответственностью приложения.
