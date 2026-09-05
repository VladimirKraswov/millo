# Millo

Millo — приложение оператора GRBL-станка для macOS и Linux: подготовка G-code,
рабочий ноль, щуп и карта высот, выполнение и восстановление задания.

**Alpha.** Совместимость с оборудованием пока не проверена исчерпывающе.
Начинайте без инструмента и шпинделя, оставайтесь у станка и сохраняйте
независимый доступ к отключению питания. Программная кнопка не заменяет E-stop.

[Сайт и загрузки](https://millo-cnc.ru) ·
[Исходный код и релизы](https://github.com/VladimirKraswov/millo)

## Рабочий процесс

Откройте файл или создайте задание через «Создать». Проверьте траекторию,
подключите станок, выставьте рабочий ноль и при необходимости примените карту.
Карточка задания показывает следующий шаг подготовки, запуска и повторения.

- G-code: редактор с подсветкой, 3D-preview, выбор строки, GRBL Check,
  проверка движения, обработка и управляемая приложением смена инструмента.
- Станок: профили, настройки из GRBL, Jog/continuous/keyboard, homing,
  G54–G59, объявленные в профиле выходы и опциональная ось A.
- Поверхность: Z-probe, сетка высот, продолжение измерения и компенсация.
- Подготовка: библиотека фрез, SVG/PNG, выравнивание, Gerber/Excellon PCB.
- Обслуживание: журнал с экспортом, восстановление, локальная русская справка,
  консоль, стандартные и внешние плагины.

GRBL Check не двигает оси. «Проверка движения» физически выполняет траекторию
без инструмента. «Обработка» работает с материалом. Возможности зависят от
диалекта, оборудования и профиля станка.

## Разработка

Rust владеет доменными правилами и serial-портом; Tauri связывает ядро с React
и Three.js. CAM и sender доступны через типизированные сервисы. Даже экспертные
команды используют общий actor, не отдельный канал записи.

Нужны Node.js 22+, Rust stable и
[системные зависимости Tauri](https://v2.tauri.app/start/prerequisites/).

```bash
npm ci
npx playwright install chromium
npm run verify:product
npm run tauri dev
```

`npm run dev` — web-preview на `http://127.0.0.1:1420`, без управления
оборудованием. `?fixture=first-cut` работает только в development.
Производственная сборка не содержит fixture-режима.

```bash
npm run bundle:mac:alpha
npm run bundle:linux
npm run virtual-controller
```

Пакеты собираются на соответствующей ОС. Виртуальный контроллер — отдельный
процесс с обычным PTY serial endpoint. Alpha DMG подписан ad hoc, без notarization.

## Документация

| Вопрос | Документ |
| --- | --- |
| Назначение и критерии продукта | [PRODUCT](docs/PRODUCT.md) |
| Переработка и оставшиеся ограничения | [PRODUCT_REVIEW](docs/PRODUCT_REVIEW.md) |
| Сценарии оператора | [OPERATOR_WORKFLOW](docs/OPERATOR_WORKFLOW.md) |
| Архитектура | [ARCHITECTURE](docs/ARCHITECTURE.md) |
| Создание и подключение плагинов | [PLUGIN_DEVELOPMENT](docs/PLUGIN_DEVELOPMENT.md) |
| Внешние макросы | [EXTERNAL_PLUGINS](docs/EXTERNAL_PLUGINS.md) |
| Проверки | [TESTING](docs/TESTING.md) |
| Инструменты и PCB | [TOOL_LIBRARY](docs/TOOL_LIBRARY.md), [PCB_JOBS](docs/PCB_JOBS.md) |
| Управление | [MACHINE_CONTROL](docs/MACHINE_CONTROL.md), [OPERATOR_CONSOLE](docs/OPERATOR_CONSOLE.md) |
| Виртуальный контроллер | [VIRTUAL_CONTROLLER](docs/VIRTUAL_CONTROLLER.md) |
| Выпуск и ограничения | [ALPHA_RELEASE](docs/ALPHA_RELEASE.md), [WEBSITE_DEPLOYMENT](docs/WEBSITE_DEPLOYMENT.md) |

Накопленная история прототипа: [IMPLEMENTATION_HISTORY](docs/IMPLEMENTATION_HISTORY.md).
Это исторические заметки, не актуальная матрица гарантированной поддержки.
