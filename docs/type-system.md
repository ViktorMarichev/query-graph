# Type System

TypeScript помогает автору до регистрации, Rust остается авторитетной runtime
проверкой wire definition.

## Scalar types

| Scalar                         | Operation value   | Результат по умолчанию |
| ------------------------------ | ----------------- | ---------------------- |
| boolean                        | boolean           | boolean                |
| int32, float64                 | number            | number                 |
| int64, decimal                 | number или string | number или string      |
| string, date, dateTime, binary | string            | string                 |
| json                           | JSON value        | JSON value             |

<code>int64</code> принимает number только в безопасном диапазоне JavaScript.
Decimal string проверяется как конечная десятичная запись.

<code>integer(value)</code> поддерживает весь диапазон от
<code>Number.MIN_SAFE_INTEGER</code> до <code>Number.MAX_SAFE_INTEGER</code>,
включая значения за пределами 32 бит.

## QueryOperation

Definition передает свои параметры, projection paths и ordering names в
operation:

```ts
const operation = {
  select: ['id', 'profile.name'],
  ordering: 'nameAsc',
  parameters: {
    organisationId: 42,
  },
} as const satisfies QueryOperation<typeof definition>
```

Required parameters обязательны, optional параметры могут отсутствовать,
list parameters принимают readonly arrays.

В Node API <code>offset</code> и <code>limit</code> принимают безопасные целые
JavaScript, включая значения больше <code>2 \*\* 32 - 1</code>.
<code>offset</code> должен быть неотрицательным, <code>limit</code> — положительным.

## Nullability

Nullability выражения складывается из:

- nullable исходного поля;
- optional relation на пути к source;
- семантики функции;
- projection object presence.

Required relation под optional ancestor остается nullable. Это свойство
вычисляется из topology один раз при регистрации.

В <code>fieldType</code> флаг <code>nullable</code> типа <code>boolean</code>
означает, что поле может быть NULL. Литеральные <code>true</code> и
<code>false</code> позволяют вывести точную nullability.

Сравнения <code>eq</code>/<code>neq</code> с гарантированным <code>null</code>
(включая <code>literal(null)</code>) дают non-null результат. Тип операнда
<code>string | null</code> сам по себе не гарантирует сравнение именно с NULL:
nullable поле при сравнении со строкой может дать NULL.

## ResultOf

```ts
type Row = ResultOf<typeof definition, typeof operation>
```

<code>ResultOf</code> строит вложенный объект из выбранных projection paths.
Если operation не передана, используются поля с <code>default: true</code>.
Для composed graph выбранные batch relations добавляются как массивы либо
nullable объекты согласно cardinality.

Driver может заменить представление scalar values:

```ts
interface DriverScalars extends DefaultScalarOutputTypeMap {
  int64: bigint
  decimal: string
  dateTime: Date
}

type DriverRow = ResultOf<typeof definition, typeof operation, DriverScalars>
```

Этот mapping существует только на уровне TypeScript. Wire schema и SQL
compilation не меняются.

## Runtime checker

Rust проверяет:

- ссылки на sources, fields и parameters;
- scalar/list shape параметров;
- совместимость сравнений и функций;
- boolean тип predicates;
- orderable и groupable expressions;
- aggregate context и concrete projection types.

Ошибки собираются в diagnostics с location исходного definition.
