# Выпуск npm-пакетов

Пакеты выпускаются с одной версией:

- `@query-graph/core`;
- `@query-graph/dsl-object`;
- `@query-graph/core-win32-x64-msvc`;
- `@query-graph/core-darwin-x64`;
- `@query-graph/core-darwin-arm64`;
- `@query-graph/core-linux-x64-gnu`.

Версия в корневом `package.json` является источником истины. Скрипт
`scripts/sync-version.mjs` синхронизирует npm workspace, оба Cargo manifest и
`Cargo.lock`.

## Первичная настройка npm

1. Создать или получить права на npm organization `@query-graph`.
2. Один раз зарегистрировать все перечисленные пакеты. npm не позволяет создать
   новый пакет через Trusted Publishing, поэтому для bootstrap-публикации нужен
   granular access token с минимальными правами.
3. Для каждого пакета в npm открыть `Settings -> Trusted Publisher` и указать:
   - organization/user: `ViktorMarichev`;
   - repository: `query-graph`;
   - workflow: `CI.yml`;
   - environment оставить пустым.
4. После проверки Trusted Publishing отозвать bootstrap-токен. Постоянный
   `NPM_TOKEN` в GitHub Actions не требуется.

Первую регистрацию лучше сделать prerelease-версией, например
`1.0.0-next.0`, с dist-tag `next`. После настройки trusted publishers можно
выпустить стабильную `1.0.0`.

## Стабильный выпуск

Рабочее дерево должно быть чистым, а изменения должны находиться в `main`.

```bash
yarn release:check
npm version patch
git push origin main --follow-tags
```

Вместо `patch` можно использовать `minor` или `major`. `npm version`:

1. выполняет все release checks;
2. меняет версию корневого npm-пакета;
3. синхронизирует DSL, Cargo manifests и `Cargo.lock`;
4. создает commit и тег `v<version>`.

Push тега запускает CI. Workflow проверяет точное совпадение тега и версии,
собирает native bindings для всех поддерживаемых платформ и публикует пакеты с
dist-tag `latest`.

## Prerelease

Первый prerelease следующей minor-версии:

```bash
npm version preminor --preid=next
git push origin main --follow-tags
```

Следующий prerelease той же версии:

```bash
npm version prerelease --preid=next
git push origin main --follow-tags
```

Версия с prerelease-суффиксом публикуется с dist-tag `next`, поэтому она не
заменяет стабильную версию при обычной установке.

## Проверки без выпуска

Полная локальная проверка:

```bash
yarn release:check
```

Только согласованность версий:

```bash
yarn check:versions
```

Содержимое архивов без публикации:

```bash
npm pack --dry-run --ignore-scripts
npm pack ./packages/dsl-object --dry-run
```

Опубликованная npm-версия неизменяема. Ошибка исправляется новой patch-версией;
повторно публиковать тот же номер версии не следует.
