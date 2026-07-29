# `@query-graph/dsl-object`

Объектный authoring DSL для [`query-graph`](https://www.npmjs.com/package/query-graph).
Он формирует семантическое определение графа без SQL; валидация, планирование и
компиляция остаются в Rust.

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

Пакет не содержит собственной реализации и напрямую переэкспортирует канонический
объектный API из `query-graph/dsl`. Поэтому у него нет второй копии фабрик,
валидации конфигурации или типов.

Структурные фабрики принимают объектные конфигурации и отклоняют неизвестные поля:

- `relation({ name, from, to, on, ...options })`;
- `constraint({ predicate, when? })`;
- `project({ path, expression, default? })`;
- `dimension({ path, expression, default? })`;
- `measure({ path, expression, default? })`.

Позиционные сигнатуры сохранены отдельно в `query-graph/definition` только как
адаптер совместимости. Они не переэкспортируются этим пакетом.
