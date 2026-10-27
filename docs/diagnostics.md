# Diagnostics

Ошибки Node API представлены как <code>QueryGraphError</code>:

```ts
try {
  registerDefinition(definition)
} catch (error) {
  if (error instanceof Error && error.name === 'QueryGraphError') {
    console.table(error.issues)
  }
}
```

Поля ошибки:

- <code>name</code>: всегда <code>QueryGraphError</code>;
- <code>phase</code>: definition, mapping, composition, operation или sql;
- <code>code</code>: основной код ошибки;
- <code>issues</code>: все diagnostics текущей фазы.

Каждый diagnostic содержит stable code, location и message. Location указывает
на wire path, например
<code>relations[1].selection.orderBy[0].expression</code>.

## Границы проверки

JavaScript DSL выполняет shallow validation сразу:

- configuration должна быть object;
- неизвестные ключи отклоняются;
- boolean и enum options проверяются до построения wire object;
- пустые обязательные строки отклоняются.

Rust выполняет semantic validation:

- уникальность и topology;
- ссылки и expression scope;
- scalar types и parameter shapes;
- projection conflicts и visibility;
- aggregate semantics;
- mapping completeness;
- operation select, ordering и parameter values.

Такое разделение дает ранние ошибки опечаток, не дублируя planner rules в
JavaScript.

Циклические зависимости relation predicates через <code>exists</code> отклоняются
при регистрации definition: <code>invalidExistsContext</code> указывает на
<code>relations[i].on</code>, замыкающий цикл. Параметры, необходимые вложенным
<code>exists</code>, проверяются в фазе <code>operation</code>; отсутствующий
параметр получает <code>missingParameter</code> с location
<code>parameters.&lt;name&gt;</code>.

Статические параметры batch relation проверяются при <code>composeGraph</code>:
ошибки относятся к фазе <code>composition</code>, в том числе для имен параметров
с квадратными скобками. Параметр batch key исключается из этой проверки по точному
имени; значения ключей проверяются позже, при <code>compileBatch</code>, в фазе
<code>operation</code>.

## Совместимость diagnostics

Wire schema версии 12 добавляет grouped-параметры и scoped-шаблоны `allGroups`.
Сохранённые raw definitions предыдущей версии требуют обновления; обычные
DSL-фабрики формируют актуальную версию автоматически. JS DSL и native binding
должны быть из одного выпуска пакета.

Невалидные данные группы получают `invalidParameterType` с точным путём,
например `parameters.characteristics[0].values[1]`. Ссылка вне своего шаблона
получает `unknownGroupScope`; повторное объявление активного scope получает
`shadowedGroupScope`. `allGroups` вне constraint predicate получает
`invalidAllGroupsContext`. Эти проверки выполняются и для raw wire definitions.

Для прежних ошибок сохраняются diagnostic
codes и locations. Message предназначен человеку и не должен использоваться как
machine-readable contract.
