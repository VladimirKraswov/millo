import type { InstalledScriptPlugin } from "../../shared/scriptPlugins";

export const previewFixtureScriptPlugins: readonly InstalledScriptPlugin[] = [
  {
    digest: "7b4dffb84ef0d0ebc577ffe954071b488bbb77f554de42fd5cf8e753ba1264e4",
    enabled: true,
    bundled: true,
    grantedCapabilities: [
      "ui.contribute",
      "machine.jog",
      "machine.coordinates",
      "jobs.create",
    ],
    package: {
      packageVersion: 1,
      manifest: {
        manifestVersion: 1,
        apiVersion: 1,
        id: "millo.operator-macros",
        name: "Операторские макросы",
        version: "1.0.0",
        description: "Безопасные команды для подготовки задания и рабочего нуля.",
        capabilities: {
          required: [
            "ui.contribute",
            "jobs.create",
            "machine.jog",
            "machine.coordinates",
          ],
          optional: ["machine.read"],
        },
      },
      commands: [
        {
          id: "boundary-check",
          title: "Проверить границу",
          description: "Создаёт spindle-free программу для Check или Air run.",
          icon: "scan-line",
          surface: "workspaceTools",
          fields: [],
          requiredCapabilities: ["jobs.create"],
        },
        {
          id: "raise-z",
          title: "Поднять Z",
          description: "Один защищённый jog.",
          icon: "arrow-up-from-line",
          surface: "machinePanel",
          fields: [],
          requiredCapabilities: ["machine.jog"],
        },
        {
          id: "return-z-zero",
          title: "Вернуть Z в ноль",
          description: "Возвращает Z к рабочему нулю.",
          icon: "locate-fixed",
          surface: "machinePanel",
          fields: [],
          requiredCapabilities: ["machine.coordinates"],
        },
        {
          id: "set-z-zero",
          title: "Z0 здесь",
          description: "Записывает текущую позицию как Z0.",
          icon: "crosshair",
          surface: "machinePanel",
          fields: [],
          requiredCapabilities: ["machine.coordinates"],
        },
      ],
      source:
        'fn run(command, input, machine) {\n  return #{ kind: "notice", title: "Fixture", message: command, tone: "success" };\n}',
    },
  },
];
