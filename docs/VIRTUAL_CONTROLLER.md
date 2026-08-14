# Виртуальный контроллер Millo VMC-3

`millo-virtual-controller` — отдельный процесс, который ведёт себя как GRBL 1.1
контроллер и публикует псевдотерминал (`/dev/ttys...`) как последовательный
порт. Millo не загружает симулятор и не выбирает отдельный режим работы: для
desktop backend это обычный `SerialTransport` со скоростью 115200 бод.

## Запуск

Разовый запуск из репозитория:

```bash
npm run virtual-controller
```

Команда печатает путь созданного порта. Пока процесс работает, этот порт
появляется в Millo с названием `Millo VMC-3 GRBL Controller` и проходит фильтр
`Только вероятные GRBL`.

Постоянная пользовательская установка на macOS:

```bash
npm run virtual-controller:install:mac
```

Скрипт собирает release binary, устанавливает его в
`~/Library/Application Support/Millo/Virtual Controller` и создаёт независимый
LaunchAgent `io.millo.virtual-controller`. Логи находятся в
`~/Library/Logs/Millo/virtual-controller.log`.

## Serial boundary

На Unix процесс создаёт raw PTY. Slave endpoint регистрируется в общем каталоге
внешних serial endpoint'ов `millo-serial`; запись содержит только обычные
метаданные порта: path, manufacturer, product и serial number. В ней нет
признака `mock` или `emulator`.

macOS IOKit перечисляет только аппаратные устройства, поэтому PTY невозможно
обнаружить через `tokio_serial::available_ports()` без дополнительной
регистрации. `millo-serial` объединяет нативный список и существующие внешние
endpoint'ы, удаляет устаревшие записи и открывает зарегистрированный PTY через
raw asynchronous file I/O. Выше serial adapter оба вида устройств полностью
одинаковы.

Идентичность контроллера:

```text
Manufacturer: Millo
Product:      Millo VMC-3 GRBL Controller
Serial:       MILLO-VMC3-0001
$I:           [VER:1.1h.20260814:Millo VMC-3]
```

## Поведение прошивки

Контроллер поддерживает используемую Millo поверхность GRBL:

- realtime `?`, `!`, `~`, Soft Reset, Jog Cancel и feed/rapid/spindle override;
- `$I`, `$$`, `$G`, `$#`, `$X`, `$C` и проверяемые записи `$n=value`;
- G54-G59, `G10 L20`, рабочие и машинные координаты;
- G0/G1 и G2/G3 в G17/G18/G19, IJK/R, полные окружности и винтовые дуги;
- G20/G21, G90/G91, G93/G94, dwell, spindle/coolant/modal telemetry;
- `$J` в абсолютном и относительном режиме;
- Z-probe ответы и PRB state для безопасных workflow fixtures;
- status поля `MPos`, `WPos`, `FS`, `Bf`, `Ov`, `Ln` и `A`.

Движение интегрируется шагом 10 ms в реальном времени. Максимальные скорости
берутся из `$110-$112`, ускорения — из `$120-$122`. Jog и program motion имеют
разгон и торможение; Hold сначала замедляет движение, Resume снова разгоняет,
Jog Cancel останавливает в промежуточной координате, Reset сбрасывает очередь.
Коллинеарные G1-блоки проходят границу строки без ложной остановки. На углах и
при смене типа движения planner консервативно тормозит до нуля.

## Что проверяет симуляция

VMC-3 подходит для проверки serial lifecycle, Inspector, профиля, Jog,
траектории, sender FIFO, Check, Hold/Resume/Reset, WCS, probing, heightmap и
восстановления UI. Тот же G-code проходит через production parser, policy,
authorization и sender.

Симулятор не доказывает физическую безопасность. Он не моделирует электрические
помехи, пропуск шагов, люфт, упругость, крутящий момент, режущие силы, реальную
геометрию инструмента и столкновения. Успешный виртуальный прогон не заменяет
проверку нулей, заготовки, инструмента и доступности аварийного отключения.

## Regression tests

```bash
cargo test -p millo-mock
cargo test -p millo-serial
cargo test -p millo-virtual-controller
```

Сквозной fixture создаёт PTY, обнаруживает его через public serial discovery,
читает `$I`, выполняет `$J`, проверяет промежуточную ускоряющуюся позицию и
подачу, затем точную конечную координату и возврат в `Idle`.
