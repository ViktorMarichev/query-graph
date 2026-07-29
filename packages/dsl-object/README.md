# `@query-graph/dsl-object`

Объектный authoring DSL для [`@query-graph/core`](https://www.npmjs.com/package/@query-graph/core).
Он формирует семантическое определение графа без SQL; валидация, планирование и
компиляция остаются в Rust.

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
    project({
      path: 'id',
      expression: users.field('id'),
      default: true,
    }),
    project({
      path: 'profile.displayName',
      expression: profiles.field('displayName'),
      default: true,
    }),
  ],
})

const graph = registerDefinition(definition)
```

Пакет не содержит собственной реализации и напрямую переэкспортирует канонический
объектный API из `@query-graph/core/dsl`. Поэтому у него нет второй копии фабрик,
валидации конфигурации или типов.

Структурные фабрики принимают объектные конфигурации и отклоняют неизвестные поля:

- `relation({ name, from, to, on, ...options })`;
- `constraint({ predicate, when? })`;
- `project({ path, expression, default? })`;
- `dimension({ path, expression, default? })`;
- `measure({ path, expression, default? })`.

Позиционные сигнатуры сохранены отдельно в `@query-graph/core/definition` только как
адаптер совместимости. Они не переэкспортируются этим пакетом.
