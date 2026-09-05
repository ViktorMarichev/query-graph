# SQL Compilation

Definition не содержит физическую схему БД. Mapping добавляется после
регистрации:

```ts
const graph = registerDefinition(definition).withRelationalMapping({
  sources: {
    users: {
      table: { catalog: 'app', schema: 'dbo', name: 'Users' },
      columns: { displayName: 'display_name' },
    },
  },
})
```

Можно передать несколько fragments через
<code>withRelationalMappings</code>. Одинаковые mappings объединяются,
конфликтующие table/column names возвращают ошибку фазы mapping.

## Диалекты

```ts
const sqlServer = graph.compileSqlServer(operation, { version: '2022' })
const oracle = graph.compileOracle(operation, { version: '19c' })
```

Поддерживаемые версии:

- SQL Server: 2008, 2012, 2016, 2019, 2022;
- Oracle: 11g, 12c, 19c, 21c, 23ai.

Version capabilities проверяются до возврата SQL. Например, pagination и
<code>firstBy</code> могут требовать более новую версию диалекта.

Semantic functions рендерятся с одинаковым поведением через разные конструкции
диалектов:

| DSL                              | Oracle                        | SQL Server                        |
| -------------------------------- | ----------------------------- | --------------------------------- |
| <code>divide(left, right)</code> | <code>left / right</code>     | дробное <code>left / right</code> |
| <code>truncate(value)</code>     | <code>TRUNC(value)</code>     | <code>ROUND(value, 0, 1)</code>   |
| <code>modulo(left, right)</code> | <code>MOD(left, right)</code> | <code>left % right</code>         |

SQL Server compiler принудительно переводит <code>divide</code> в дробную
арифметику, чтобы два integer operands не дали integer division.

В SQL Server 2012 и новее <code>concat</code> с одним аргументом компилируется
с дополнительной пустой строкой <code>''</code>. Списки длиннее 254 аргументов
разделяются на вложенные вызовы с сохранением порядка. Промежуточные группы
используют <code>nvarchar(max)</code>, чтобы избежать обрезания строк при
объединении. <code>NULL</code> обрабатывается как пустая строка согласно
[семантике CONCAT в SQL Server](https://learn.microsoft.com/en-us/sql/t-sql/functions/concat-transact-sql).

## Bindings

Compiler никогда не вставляет operation values в SQL. Каждый binding содержит:

- SQL placeholder name;
- имя parameter;
- scalar type;
- index элемента для list parameter.

Повторное использование parameter переиспользует binding. Пустой list
компилируется в ложное условие без placeholders.

## Result metadata

SQL columns получают технические aliases <code>c0</code>,
<code>c1</code> и так далее. <code>statement.columns</code> хранит logical path,
scalar type, nullability и relation path. <code>statement.objects</code> хранит
presence columns для nullable объектов.

Executor должен опираться на metadata, а не анализировать SQL.

## Граница исполнения

query-graph не владеет:

- connection pool и transaction;
- timeout, cancellation и retry;
- tracing и logging;
- преобразованием driver-specific bind values;
- hydration rows и batch attachment.

Эти обязанности остаются у приложения. Один и тот же graph можно использовать
с разными executors, пока они соблюдают контракт statement metadata.
