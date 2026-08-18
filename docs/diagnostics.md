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

## Совместимость diagnostics

Wire schema остается версии 10. В совместимых выпусках сохраняются diagnostic
codes и locations. Message предназначен человеку и не должен использоваться как
machine-readable contract.
