# Composition

В query-graph есть два разных вида композиции.

- <code>GraphModule</code> расширяет один SQL-граф до регистрации.
- <code>batchRelation</code> связывает уже зарегистрированные графы несколькими
  SQL-запросами.

## Когда использовать GraphModule

GraphModule подходит, когда данные безопасно входят в один relational plan:
to-one связи, общие constraints, набор projection fields или ordering.

Many-связь в одном SQL может размножить root-строки и нарушить пагинацию. Для
таких полей нужен batch.

## Batch query

Child-модуль один раз закрепляет контракт ключа:

```ts
const itemIds = requiredListParameter('itemIds', 'int64')

const childrenQuery = batchQuery({
  graph: childrenGraph,
  key: {
    path: 'itemId',
    parameter: itemIds,
  },
})
```

<code>key.path</code> должен быть projection child-графа.
<code>key.parameter</code> должен быть list parameter того же scalar type.

## Batch relation

Root-граф подключает child query:

```ts
const children = batchRelation({
  name: 'children',
  from: 'id',
  query: childrenQuery,
  cardinality: 'many',
  parameters: {
    state: 'active',
  },
})

const graph = composeGraph({
  root: rootGraph,
  relations: [children],
})
```

<code>from</code> является projection path root-графа. Static parameters
проверяются при композиции. Сам list parameter ключей в <code>parameters</code>
передавать нельзя: compiled plan заполняет его автоматически.

## Компиляция плана

```ts
const plan = graph.compileOraclePlan({
  select: ['id', 'children.id', 'children.name'],
  parameters: rootParameters,
})
```

Planner:

1. отделяет root selection от child selections;
2. добавляет root key в select, если потребитель его не запросил;
3. добавляет child key в child select;
4. компилирует только root statement;
5. возвращает metadata выбранных batch steps.

Batch relation не компилируется и не выполняется, если ее поля не выбраны.

## Deferred batch

После выполнения root executor собирает уникальные ключи и компилирует один
child statement:

```ts
for (const batch of plan.batches) {
  const keys = collectKeys(rootRows, batch.parentKey)
  const statement = plan.compileBatch(batch.name, keys)
  const childRows = await driver.query(statement.sql, bind(statement, keys))
  attach(rootRows, childRows, batch)
}
```

При <code>cardinality: 'many'</code> executor присоединяет массив. При
<code>cardinality: 'one'</code> он присоединяет объект или NULL. Cardinality
описывает результат, а не количество SQL-запросов.

Metadata сообщает <code>parentKeyInjected</code> и
<code>childKeyInjected</code>. Executor может удалить технически добавленные
ключи после сборки результата.
