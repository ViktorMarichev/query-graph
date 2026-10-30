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

Grouped list parameters выводятся как:

```ts
readonly {
  key: ScalarParameterValue<KeyType>
  values: readonly ScalarParameterValue<ValueType>[]
}[]
```

Например, <code>requiredGroupsParam('filters', { key: 'string', value: 'int64' })</code>
требует <code>parameters.filters</code> типа
<code>readonly { key: string; values: readonly (number | string)[] }[]</code>.
Optional grouped parameter может отсутствовать, но не заменяться на <code>null</code>.
Оба массива принимают readonly tuples, в том числе из <code>as const</code>.

Контейнер групп, отдельная группа и контейнер <code>values</code> не могут быть
<code>null</code>. Ключ и элементы списка следуют обычному scalar contract:
<code>null</code> запрещен для всех типов, кроме <code>json</code>. JSON сохраняет
существующую семантику JSON values, включая <code>null</code> и вложенные структуры.
Rust проверяет наличие <code>key</code>/<code>values</code>, неизвестные поля
группы и scalar types, включая ограничения числовых значений.

Grouped parameters сохраняют эти типы при композиции GraphModule и могут быть
статическими параметрами дочернего <code>batchRelation</code>. Ключ
<code>batchQuery</code> по-прежнему требует обычный list parameter, не grouped list.

В callback <code>allGroups</code> ключ имеет scalar type <code>KeyType</code>,
а <code>GroupValuesRef&lt;ValueType&gt;</code> принимается только как второй аргумент
<code>inParameter</code>/<code>inParam</code>. TypeScript исключает эту reference
из scalar expressions и root parameters; Rust проверяет lexical scope и
совместимость типов сравнения и membership.

Для grouped bindings <code>path</code> указывает на значение внутри operation
parameter: <code>[groupIndex, 'key']</code> или
<code>[groupIndex, 'values', valueIndex]</code>. Scalar bindings остаются без
<code>index</code>/<code>path</code>, обычные list bindings сохраняют
<code>index</code>. Поля <code>index</code> и <code>path</code> взаимоисключающие.
Это же правило действует для результатов <code>compileBatch</code>.

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

<code>allGroups</code> возвращает boolean с nullability predicate шаблона.
Пустой набор групп дает <code>true</code>, но при наличии групп nullable
сравнение внутри callback может дать SQL UNKNOWN. Если callback возвращает
<code>exists</code>, результат остается non-null boolean.

## ResultOf

```ts
type Row = ResultOf<typeof definition, typeof operation>
```

<code>ResultOf</code> строит вложенный объект из выбранных projection paths.
Если operation не передана, используются поля с <code>default: true</code>.
Для composed graph выбранные batch relations добавляются как массивы либо
nullable объекты согласно cardinality.

Если объект объявлен через прямое поле источника:

```ts
projectObject({ path: 'profile', presence: profiles.field('id') })
```

то внутри существующего `profile` поля этого источника больше не получают
`null` из-за optional JOIN. Например, для non-null `id` и `name` и nullable
`nickname` результат будет:

```ts
type Row = {
  profile: {
    id: number | string
    name: string
    nickname: string | null
  } | null
}
```

Сам объект остается nullable. Nullable-колонки, поля других sources и поля
вне этого объекта сохраняют прежнюю nullability. Вложенные объекты учитывают
presence родителей, но наличие родителя не гарантирует наличие другого
источника, даже если связь с ним объявлена `required`.

Вычисляемый presence, например `coalesce(profiles.field('id'), 0)` или
`isNotNull(profiles.field('id'))`, не подтверждает наличие источника.
Если тип не позволяет однозначно определить source или путь объекта,
уточнение также не применяется.

Это уточнение относится к результату после обработки `statement.objects`
потребителем. SQL и `statement.columns[].nullable` не меняются: до сборки
вложенных объектов отсутствующая строка JOIN по-прежнему дает NULL-колонки.

Driver может заменить представление scalar values:

```ts
interface DriverScalars extends Omit<DefaultScalarOutputTypeMap, 'int64' | 'decimal' | 'dateTime'> {
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
- scalar/list/groupedList shape параметров и key type grouped parameters;
- lexical scopes ключей и списков групп, допустимость binder в constraint context;
- совместимость сравнений и функций;
- boolean тип predicates;
- orderable и groupable expressions;
- aggregate context и concrete projection types.

Ошибки собираются в diagnostics с location исходного definition.
