export type ToolKind =
  | "flatEndMill"
  | "ballNose"
  | "vBit"
  | "engraving"
  | "drill"
  | "surfacing";

export interface ToolReference {
  readonly manufacturer: string;
  readonly product: string;
  readonly url: string;
}

export interface CuttingTool {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly kind: ToolKind;
  readonly diameterMm: number;
  readonly tipDiameterMm?: number;
  readonly shankDiameterMm: number;
  readonly cuttingLengthMm: number;
  readonly fluteCount: number;
  readonly includedAngleDegrees?: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly spindleRpm: number;
  readonly stepdownMm: number;
  readonly stepoverPercent: number;
  readonly factoryPreset: boolean;
  readonly reference?: ToolReference;
}

export interface CuttingToolDraft {
  readonly name: string;
  readonly description: string;
  readonly kind: ToolKind;
  readonly diameterMm: number;
  readonly tipDiameterMm?: number;
  readonly shankDiameterMm: number;
  readonly cuttingLengthMm: number;
  readonly fluteCount: number;
  readonly includedAngleDegrees?: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly spindleRpm: number;
  readonly stepdownMm: number;
  readonly stepoverPercent: number;
}

export interface ToolLibraryState {
  readonly tools: readonly CuttingTool[];
  readonly revision: number;
}

export const emptyToolLibrary: ToolLibraryState = Object.freeze({
  tools: Object.freeze([]),
  revision: 0,
});

export const toolKindLabels: Readonly<Record<ToolKind, string>> = Object.freeze({
  flatEndMill: "Плоская",
  ballNose: "Шаровая",
  vBit: "V-фреза",
  engraving: "Гравировальная",
  drill: "Сверло",
  surfacing: "Торцевая",
});

export interface ToolKnowledge {
  readonly bestFor: readonly string[];
  readonly cautions: readonly string[];
}

const knowledge: Readonly<Record<ToolKind, ToolKnowledge>> = Object.freeze({
  flatEndMill: Object.freeze({
    bestFor: Object.freeze([
      "Пазы, карманы и контуры с плоским дном",
      "Черновое снятие материала и раскрой деталей",
      "Дерево, пластик и подходящие сплавы при корректном режиме",
    ]),
    cautions: Object.freeze([
      "Внутренний радиус не может быть меньше радиуса фрезы",
      "Длинный вылет усиливает прогиб и вибрацию",
      "Для чистовой 3D-поверхности обычно лучше шаровая фреза",
    ]),
  }),
  ballNose: Object.freeze({
    bestFor: Object.freeze([
      "Чистовая обработка рельефов и плавных 3D-поверхностей",
      "U-образные канавки и скруглённые переходы",
      "Финиш после чернового прохода плоской фрезой",
    ]),
    cautions: Object.freeze([
      "На плоскости оставляет гребешки, поэтому нужен малый stepover",
      "Центр торца режет медленно и склонен к трению",
      "Не лучший выбор для быстрого удаления большого объёма материала",
    ]),
  }),
  vBit: Object.freeze({
    bestFor: Object.freeze([
      "V-carving, надписи и декоративные линии переменной ширины",
      "Фаски и выборка острых внутренних углов",
      "60° для более тонких деталей, 90° для широких надписей и фасок",
    ]),
    cautions: Object.freeze([
      "Ширина реза напрямую зависит от фактической глубины и рабочего нуля Z",
      "Глубокая канавка ухудшает отвод стружки и сильнее нагружает инструмент",
      "Кончик чувствителен к биению шпинделя и ошибке высоты поверхности",
    ]),
  }),
  engraving: Object.freeze({
    bestFor: Object.freeze([
      "Тонкая маркировка, мелкие символы и неглубокие контуры",
      "PCB и небольшие декоративные детали подходящего материала",
      "Работа с очень малой глубиной при точном нуле Z",
    ]),
    cautions: Object.freeze([
      "Тонкий кончик легко повредить глубокой подачей или ударом",
      "Неровная поверхность заметно меняет ширину линии",
      "Перед запуском полезны карта высот или предварительное выравнивание",
    ]),
  }),
  drill: Object.freeze({
    bestFor: Object.freeze([
      "Отверстия по Excellon с диаметром, совпадающим со сверлом",
      "Монтажные и переходные отверстия печатных плат",
      "Осевое погружение с отдельной подачей Z",
    ]),
    cautions: Object.freeze([
      "Не заменяйте сверло фрезой большего диаметра, чем отверстие",
      "Тонкие PCB-сверла чувствительны к биению и боковой нагрузке",
      "Проверьте глубину с учётом текстолита и подложки",
    ]),
  }),
  surfacing: Object.freeze({
    bestFor: Object.freeze([
      "Выравнивание жертвенного стола и деревянных панелей",
      "Широкие неглубокие проходы с высокой производительностью",
      "Проверка перпендикулярности шпинделя по следам соседних проходов",
    ]),
    cautions: Object.freeze([
      "Используйте только тонкий съём и надёжно закреплённую поверхность",
      "Ступеньки между полосами обычно указывают на необходимость tram шпинделя",
      "Проверьте допустимые RPM, материал и направление пластин производителя",
    ]),
  }),
});

export const toolKnowledge = (kind: ToolKind): ToolKnowledge => knowledge[kind];

export const supportsSurfacing = (tool: CuttingTool): boolean =>
  tool.kind === "flatEndMill" || tool.kind === "surfacing";

export const effectiveCuttingDiameterMm = (
  tool: Pick<CuttingTool, "diameterMm" | "tipDiameterMm" | "includedAngleDegrees">,
  depthMm: number,
): number | undefined => {
  if (!Number.isFinite(depthMm) || depthMm < 0) return undefined;
  if (tool.includedAngleDegrees !== undefined) {
    const tip = tool.tipDiameterMm ?? 0;
    return Math.min(
      tool.diameterMm,
      Math.max(tip, tip + 2 * depthMm * Math.tan(tool.includedAngleDegrees * Math.PI / 360)),
    );
  }
  return tool.tipDiameterMm === undefined ? tool.diameterMm : undefined;
};

export const draftFromTool = (tool: CuttingTool): CuttingToolDraft => ({
  name: tool.name,
  description: tool.description,
  kind: tool.kind,
  diameterMm: tool.diameterMm,
  tipDiameterMm: tool.tipDiameterMm,
  shankDiameterMm: tool.shankDiameterMm,
  cuttingLengthMm: tool.cuttingLengthMm,
  fluteCount: tool.fluteCount,
  includedAngleDegrees: tool.includedAngleDegrees,
  feedMmPerMin: tool.feedMmPerMin,
  plungeMmPerMin: tool.plungeMmPerMin,
  spindleRpm: tool.spindleRpm,
  stepdownMm: tool.stepdownMm,
  stepoverPercent: tool.stepoverPercent,
});

export const newToolDraft = (): CuttingToolDraft => ({
  name: "Новая фреза",
  description: "Пользовательский инструмент. Уточните геометрию и режимы по паспорту производителя.",
  kind: "flatEndMill",
  diameterMm: 3.175,
  tipDiameterMm: undefined,
  shankDiameterMm: 3.175,
  cuttingLengthMm: 12,
  fluteCount: 2,
  feedMmPerMin: 400,
  plungeMmPerMin: 120,
  spindleRpm: 16_000,
  stepdownMm: 0.8,
  stepoverPercent: 35,
});
