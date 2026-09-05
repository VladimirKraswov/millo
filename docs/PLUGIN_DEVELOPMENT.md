# Разработка плагинов Millo

Документ описывает фактический API Millo v1. В проекте есть два разных уровня
расширений. Выбор уровня является частью модели доверия, а не вопросом удобства.

## Какой тип выбрать

| Тип | Для чего | Код | Установка | Доступ |
| --- | --- | --- | --- | --- |
| Встроенный trusted plugin | Системная функция с полноценным React UI | TypeScript/React, собирается вместе с Millo | Через исходный код и релиз Millo | Только выданные typed capabilities |
| Внешний plugin/macro | Пользовательская команда или генератор задания | JSON + Rhai | Импорт `.millo-plugin` оператором | Одна декларативная команда и одно typed действие за запуск |

Внешний пакет не может добавлять React, HTML, JavaScript, native library или
произвольный Tauri command. Для внешнего UI используются объявленные команды,
поля и одна из двух поверхностей. Это сохраняет возможность проверить пакет до
включения и не даёт стороннему коду получить serial/sender в обход ядра.

## Встроенный TypeScript plugin

### Структура

```text
src/plugins/my-plugin/
  createMyPlugin.tsx       # manifest и activate/deactivate
  MyPluginPanel.tsx        # UI, не знает про Tauri
  createMyPlugin.test.tsx  # grants, registration, unload
```

Плагин импортирует host contract только из `src/plugin-sdk`. Прямой импорт
`platform/plugins`, `platform/extensions`, `src/api` или `@tauri-apps/*` из
production-файлов плагина запрещён `npm run test:architecture`.

```tsx
import { useSyncExternalStore } from "react";
import {
  createPluginManifest,
  definePlugin,
  uiSlots,
  type InMemoryPluginModule,
  type PluginMachineReadCapability,
} from "../../plugin-sdk";

export const SAMPLE_PLUGIN_ID = "dev.example.sample";

function SamplePanel({ machine }: { machine: PluginMachineReadCapability }) {
  const snapshot = useSyncExternalStore(
    machine.subscribe, machine.current, machine.current,
  );
  return <output>{snapshot.machine.reportedMode}</output>;
}

export function createSamplePlugin(): InMemoryPluginModule {
  return definePlugin({
    manifest: createPluginManifest({
      id: SAMPLE_PLUGIN_ID,
      name: "Sample",
      version: "1.0.0",
      capabilities: {
        required: ["ui.contribute", "machine.read"],
        optional: [],
      },
    }),
    activate(context) {
      if (!context.ui || !context.machineRead) {
        throw new Error("required capabilities are unavailable");
      }
      const registration = context.ui.register({
        id: `${SAMPLE_PLUGIN_ID}.panel`,
        slot: uiSlots.controlMachine,
        order: 100,
        render: () => <SamplePanel machine={context.machineRead!} />,
      });
      return () => registration.dispose();
    },
  });
}
```

`definePlugin` проверяет manifest при загрузке модуля. ID плагина состоит из
строчных ASCII-сегментов, разделённых `.` или `-`; ID каждого UI contribution
начинается с `${pluginId}.`. `apiVersion` и `manifestVersion` подставляются
helper-функцией и не должны задаваться вручную.

### Подключение

1. Добавьте factory в `bundledPlugins` внутри `bootstrapPluginHost`.
2. Добавьте grant для того же ID в `CapabilityGrantStore` composition root.
3. Для новой host capability сначала создайте узкий platform-neutral gateway,
   policy в Rust и тесты. Не передавайте плагину Tauri `invoke` или actor.
4. Проверьте отсутствие функции без grant, её работу с grant и закрытие proxy
   после `unload`.

Пример регистрации уже есть в `src/app/useWorkstation.ts`; эталонные реализации находятся в
`src/plugins/image-to-gcode` и `src/plugins/spoilboard-surfacing`.

### Диалоги плагина

Встроенные плагины импортируют `DialogSurface` из `src/plugin-sdk`, не создают
собственный глобальный Escape listener и не управляют фокусом вручную.
`DialogHost` уже установлен в корне приложения. Он учитывает особенности
WebKit и возвращает фокус на кнопку открытия, даже если клик её не сфокусировал.

```tsx
import { DialogSurface } from "../../plugin-sdk";

<DialogSurface
  aria-labelledby="my-job-title"
  onDismiss={onClose}
  dismissible={!exporting}
  className="my-job-dialog"
>
  <header>
    <h2 id="my-job-title">Моё задание</h2>
    <button type="button" disabled={exporting} onClick={onClose} aria-label="Закрыть">
      <X size={18} aria-hidden="true" />
    </button>
  </header>
  {children}
</DialogSurface>
```

`X` импортируется из `lucide-react`. Внешний backdrop и layout остаются за
плагином; `DialogSurface` не добавляет визуальную тему. По умолчанию Tab остаётся
в диалоге. `modal={false}` отключает удержание клавиатурного фокуса. Если панель
должна пропускать клики к остальному интерфейсу, её backdrop также нужно
спроектировать соответствующим образом: ARIA-атрибут сам этого не делает.
При открытии фокус получает сама поверхность, а не кнопка запуска. Escape
закрывает только верхний диалог и соблюдает актуальный `dismissible`.
`onDismiss` закрывает UI; остановка станка всегда должна быть отдельной typed
командой. Выгрузка плагина очищает и обработчики диалога, и регистрации capability.

Этот React-компонент относится только к trusted-плагинам. Формат внешних
Rhai-пакетов и границы выдаваемых им возможностей от этого не меняются.

### Границы модуля

- Composition root создаёт gateway/service и выдаёт capability; плагин их не
  конструирует и не импортирует Tauri.
- UI-компонент плагина получает узкие capability props и хранит только локальное
  состояние формы. Парсинг, CAM, профиль станка и safety policy остаются в core.
- Повторно используемый алгоритм сначала добавляется в Rust/host service с
  fixture-тестами, затем вызывается плагином. Нельзя прятать доменную реализацию
  внутри React-модалки.
- Проверка capability выполняется host proxy и Rust use case. Видимость кнопки
  является UX, но не авторизацией.
- Каждый `register`/`subscribe` обязан принадлежать lifecycle scope. Loader
  закрывает scope до `deactivate`, поэтому отложенный callback не должен
  рассчитывать на ещё живой proxy.

Такое разделение позволяет заменить UI плагина, повторно использовать ядро из
другого плагина и тестировать machine behavior без React и Tauri.

### Trusted capabilities v1

Снимки `machine.read` полностью immutable, включая homing, pins, overrides
и буферы. Храните настройки плагина отдельно, не дописывайте поля в snapshot.
Ошибка одного подписчика не прерывает доставку другим; исключения обработчика
диагностики также изолируются. После unload нет новых callback. Асинхронная
подписка host сначала регистрирует listener, затем читает snapshot, чтобы не
терять обновления при открытии панели.

- `ui.contribute`: регистрация React contribution в именованном slot.
- `machine.read`: frozen snapshot и автоматически очищаемая подписка.
- `machine.jog`: один typed jog через обычный command actor.
- `machine.coordinates`: typed `setZero` и `returnToZero`.
- `jobs.create`: генерация только через Millo CAM core; открыть/сохранить можно
  только job object, ранее выданный этим core. Включает `inspectPcb` и
  `generatePcb`; Gerber/Excellon и выбранные `toolId` проверяются в Rust.
- `tools.read`: frozen библиотека инструмента и tracked subscription.

Capability proxy проверяет resource scope до и после `await`. Unload сначала
закрывает scope и подписки, затем вызывает plugin deactivation. Поздний результат
асинхронного activate не может повторно включить уже выгруженный плагин. Ошибка
рендера contribution изолируется React error boundary и не роняет App Shell.
При размонтировании composition root `PluginHost.dispose()` выгружает активные
и отменяет ещё загружающиеся плагины; повторный dispose идемпотентен.

## Внешний `.millo-plugin`

### Минимальный пакет

```json
{
  "packageVersion": 1,
  "manifest": {
    "manifestVersion": 1,
    "apiVersion": 1,
    "id": "local.example-notice",
    "name": "Example notice",
    "version": "1.0.0",
    "description": "Shows one checked message.",
    "capabilities": {
      "required": ["ui.contribute"],
      "optional": []
    }
  },
  "commands": [
    {
      "id": "show",
      "title": "Показать сообщение",
      "description": "Проверка внешнего плагина.",
      "icon": "braces",
      "surface": "workspaceTools",
      "fields": [],
      "requiredCapabilities": []
    }
  ],
  "source": "fn run(command, input, machine) { #{ kind: \"notice\", title: \"Готово\", message: \"Плагин работает\", tone: \"success\" } }"
}
```

`ui.contribute` обязателен для command package. Capabilities действия должны
быть объявлены и в manifest, и в `command.requiredCapabilities`. Если команда
вернёт jog, не объявив `machine.jog`, runtime отклонит результат. Поля бывают
`number`, `boolean`, `text`; неизвестные поля, NaN/Infinity, неверный тип,
границы и слишком длинный text отклоняются до выполнения Rhai.

### Действия v1

Внешняя capability `machine.commands` разрешает действие `rawCommand`. Она не
входит автоматически в trusted TypeScript SDK: first-party React plugin должен
использовать узкий typed gateway либо отдельно спроектированный host proxy.

```rhai
// Создать программу; она будет повторно разобрана millo-gcode.
#{ kind: "createProgram", sourceName: "frame.nc", source: "G21\nM5\nM9\nM30" }

// Один guarded jog.
#{ kind: "jog", axis: "z", distanceMm: 1.0, feedMmPerMin: 100.0 }

// Установить или вернуть одну координату рабочего нуля.
#{ kind: "setZero", axis: "z" }
#{ kind: "returnZero", axis: "z", feedMmPerMin: 100.0 }

// Экспертная строка. Требует machine.commands, подтверждение оператора и
// выключенный глобальный безопасный режим.
#{ kind: "rawCommand", command: "$SD/Job.nc" }

// Сообщение без machine capability.
#{ kind: "notice", title: "Готово", message: "Операция завершена", tone: "success" }
```

Каждый запуск возвращает ровно одно действие. `sourceName` должен быть простым
именем файла без каталога. Machine action требует capability, подтверждение
оператора, привязанный профиль и проверки существующего actor use case. Сам
Rhai runtime не получает serial, sender, filesystem, network, Tauri, DOM,
`import` или `eval`. `rawCommand` не меняет эту границу: runtime возвращает
данные, host повторно валидирует строку и передаёт её actor. Разрешена одна
печатная ASCII-строка до 255 байт; `!`, `~`, Ctrl-X и multiline запрещены.

### Установка и обновление

1. Откройте `Плагины` в Millo и импортируйте файл.
2. Сверьте ID, команды, исходник, capabilities и SHA-256 digest.
3. Выберите optional grants и включите пакет.
4. После изменения или повторного импорта digest меняется, пакет выключается,
   старые grants удаляются. Его надо проверить заново.

Store ограничен 128 пакетами и 64 MiB, пишет candidate atomically и только
после успешного `fsync` публикует новое состояние в процессе. Повреждённый
primary восстанавливается из `.bak`. Import/configure/delete/execute проходят
через один execution fence: выключение или обновление не может обогнать уже
проверяемое machine action.

## Тестирование плагина

Минимальный набор для trusted plugin:

1. Без required grant `activate` не вызывается.
2. С grants contribution появляется только в заявленном slot.
3. Сгенерированный job выдан core и проходит parser fixtures.
4. Unload удаляет UI/подписки; сохранённый proxy отклоняет новый вызов.
5. Ошибка activate откатывает частичную регистрацию.

Для external package добавьте Rust fixtures в `millo-script`: schema failure,
operation budget, корректное действие, недостающая capability и повторный parse
созданного G-code.

```bash
npm run test:architecture
npm run test:ui -- --run src/platform/plugins src/plugin-sdk src/plugins
cargo test -p millo-script
npm run verify
```

## Версионирование

Изменение смысла capability, action, manifest/package field или lifecycle
является изменением API. Оно требует нового `PLUGIN_API_VERSION`, migration
note, fixtures старой версии и обновления этого документа. Добавление новой
capability в v1 допустимо только как явное разрешение с отсутствующим proxy без
grant; неизвестные capabilities всегда отклоняются.
