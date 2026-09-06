# Технический долг: 2026-09-05

Следующий аудит поведения ядра, CAM и асинхронных UI-переходов:
[PRODUCT_AUDIT_2026_09](PRODUCT_AUDIT_2026_09.md). Результаты и числа тестов
ниже относятся к предыдущему срезу, не к текущему HEAD.

Этот срез продолжает [продуктовый разбор](PRODUCT_REVIEW.md). Цель: убрать
монолиты и повторные UI-механизмы, сохранив рабочий протокол и защитные проверки.
Рефакторинг не меняет систему координат, глубину, траектории, authorization,
порядок GRBL-команд или правила возобновления. Аппаратные движения не выполнялись.

## Закрытые Пункты

| Долг | Результат | Как проверяется |
| --- | --- | --- |
| Actor смешивал API, очередь, операции и тесты в 8 940 строках | Корневой контракт менее 550 строк; client/runtime/dispatch и отдельные модули операций | Все 111 actor tests сохранены; общий fixture setup и группы по операциям |
| Tauri command-файл более 4 000 строк | Модули по предметным областям, прежние IPC names и общая AppState | Workspace tests, typecheck, Clippy; тесты IPC/request/persistence не удалены |
| Состояние и JSX смешаны в главных экранах | Отдельные координаторы приложения/задания; surface-session hook и чистая readiness projection | Render/policy tests и полные browser workflows |
| 11 462 строки CSS в одном файле | 31 тематический файл и явно упорядоченный index | При миграции проверено точное совпадение исходного текста после конкатенации; screenshots и проверки геометрии |
| Разные Escape/focus механизмы | Общие DialogHost/DialogSurface для всех feature/system-plugin диалогов | StrictMode, вложенные окна, запрет закрытия, Tab, Escape, возврат фокуса, отсутствие запуска по Enter |
| Глобальные стрелки Jog могли срабатывать внутри диалога | Общая keyboard policy уступает диалогам, вводу, меню, IME и обработанным событиям | Browser regression: фокус поверхности, input, обычная рабочая область и обработанная стрелка; keyup для остановки не фильтруется |
| Chromium не выявлял WebKit-особенности | Пять проектов: три Chromium, два WebKit | Те же сценарии без ослабления проверок; найден и исправлен возврат фокуса после pointer activation |
| Хрупкий regex для архитектурных импортов | Разбор TypeScript AST и ограничения роста координаторов/стилей | test:architecture в обычном gate |
| glib 0.18.5 передавал неизменяемый out-pointer в C | Минимальный совместимый upstream backport, один glib во всём GTK dependency graph | SHA-256 исходника, Cargo metadata, оптимизированный Linux iterator test |
| Не было реестра сторонних компонентов в сборке | Генерируемые metadata и license/copyright/notice texts включены в native resources | test:notices проверяет воспроизводимость относительно lockfiles |

## Что Намеренно Сохранено

- Единственный владелец serial-порта: Rust actor. Декомпозиция не создаёт
  дополнительных очередей записи или второго sender.
- Проверки UI и ядра не объединяются в одну клиентскую проверку: UI объясняет
  состояние, Rust повторно проверяет полномочия и свежесть перед действием.
- Фокус и закрытие окна не посылают GRBL-команд. Nonmodal-панели не удерживают
  Tab; размещение и обработка кликов backdrop остаются частью layout функции.
- Тесты разных сбоев, race conditions и режимов не удаляются как «дубли» только
  потому, что используют одинаковую заготовку.
- Координаторы остаются координаторами, а не универсальным framework. Их
  дальнейший рост ограничен исполняемыми budgets, не обещанием «писать SOLID».

## Зависимости И Ограничения

Повторный npm audit на этом срезе: 0 опубликованных уязвимостей. RustSec не
сообщает известных vulnerabilities для lockfile, но остаются 17 предупреждений
`unmaintained`, преимущественно транзитивного GTK3/Tauri стека. Отсутствие
advisory для локальной копии glib не считается доказательством исправления:
проверяются сам patch и исполняемый regression. Основания и удаление backport
описаны в [vendor/README.md](../vendor/README.md).

Список [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) содержит 524 сторонних
пакета и ссылки на включённые тексты через машиночитаемый JSON. У 45 пакетов
в опубликованном source archive нет отдельного license-text файла; они явно
перечислены, а не объявлены автоматически юридически проверенными. Реестр
включает Rust build/test и зависимости всех платформ консервативно.

Не закрываются одним рефакторингом:

- upstream maintenance GTK3/WebKitGTK: нужна поддерживаемая миграция desktop
  adapter, не добавление второй несовместимой версии glib;
- лицензия самого Millo: решение владельца; реестр зависимостей её не назначает;
- Developer ID/notarization Apple: нужны соответствующие ключи и учётная запись;
- аппаратная приёмка и физическая точность восстановления после потери питания;
- поддержка отсутствующих в parser/fixture корпусе диалектов GRBL/Gerber.

## Проверка Перед Выпуском

Локальный `verify:product` завершился успешно: 295 TypeScript-тестов,
428 Rust-тестов, 5 тестов сайта, 40 browser scenarios на Chromium/WebKit;
typecheck, production bundle budgets, rustfmt и Clippy `-D warnings` также
прошли. Проверка notices воспроизводима. Это результат macOS-прогона;
оптимизированный Linux regression проверяется отдельным CI job step.

Linux job первого CI-прогона также прошёл полностью, включая optimized glib
regression. Предупреждение о принудительной смене runtime старых Actions
устранено обновлением checkout/setup-node/upload-artifact на Node 24 версии,
закреплённые по commit SHA. Checkout не сохраняет token в git config.
Версии сверены с официальными manifests:
[checkout](https://github.com/actions/checkout/blob/v7.0.1/action.yml),
[setup-node](https://github.com/actions/setup-node/blob/v7.0.0/action.yml),
[upload-artifact](https://github.com/actions/upload-artifact/blob/v7.0.1/action.yml).

Первый macOS 14 CI выполнил Rust/Chromium, но не смог создать страницу WebKit:
`Unknown setting: PushAPIEnabled`. Playwright 1.63 выбирает для macOS 14
замороженную revision 2251 вместо 2359. CI перенесён на macOS 15; перед сценариями
теперь создаётся тестовая страница каждого browser engine. Проверки приложения
не ослаблены и не переведены в retries. Это не доказательство совместимости
native WebView со всеми старыми версиями macOS.

Повторный [CI 76d2e66](https://github.com/VladimirKraswov/millo/actions/runs/33991930220)
успешно выполнил полный gate на macOS 15 и Ubuntu 24.04, без предупреждения
о Node 20 Actions. После уточнения Keyboard Jog повторены 40 локальных
browser scenarios и typecheck; финальный commit также проходит обычный CI.

```bash
cargo fetch --locked
npm ci
npx playwright install chromium webkit
npm run test:notices
npm run verify:product
npm run test:dependencies
cargo audit
```

На Linux дополнительно:

```bash
cargo test -p millo-platform-tests --release --locked
```

Результаты конкретного релизного прогона фиксируются в release notes. Браузерные
fixtures не подключают станок. Playwright WebKit полезен для совместимости UI,
но не заменяет проверку установленного WKWebView/WebKitGTK приложения.
