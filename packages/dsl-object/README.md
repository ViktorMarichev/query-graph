# `@query-graph/dsl-object`

Объектный authoring DSL для [`query-graph`](https://www.npmjs.com/package/query-graph).
Он создаёт тот же canonical graph definition, что и `query-graph/definition`;
валидация, планирование и компиляция SQL остаются в Rust.

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
    project({
      path: 'id',
      expression: staff.field('id'),
      default: true,
    }),
    project({
      path: 'credentials.idPerson',
      expression: personStaff.field('idPerson'),
      default: true,
    }),
  ],
})

const graph = registerDefinition(definition)
```

Пакет предоставляет намеренно ограниченный authoring API, а не зеркальную копию
`query-graph/definition`. Общие фабрики sources, parameters, expressions и
композиции делегируются canonical functional implementation. Объектные аргументы
используются структурными фабриками:

- `relation({ name, from, to, on, ...options })`;
- `constraint({ name, predicate, when? })`;
- `project({ path, expression, default? })`;
- `dimension({ path, expression, default? })`;
- `measure({ path, expression, default? })`.

Wire-level `GRAPH_DEFINITION_VERSION` и дублирующий standalone `field(source,
name)` остаются в `query-graph/definition`. Расширение functional DSL не становится
частью object DSL автоматически.
