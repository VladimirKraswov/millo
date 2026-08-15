import type {
  PcbDrillGroup,
  PcbInspection,
  PcbJobSettings,
  PcbLayerRole,
  PcbSourceFile,
} from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";

export interface LocalPcbFile extends PcbSourceFile {
  readonly sizeBytes: number;
}

const allowedExtensions = new Set([
  "art", "cmp", "dim", "drl", "drd", "exc", "fab", "g1", "g2", "g3", "gba", "gbl", "gbo",
  "gbp", "gbr", "gbs", "ger", "gko", "gm1", "gm2", "gml", "grb", "gta", "gtl", "gto",
  "gtp", "gts", "mil", "ncd", "oln", "pho", "plc", "pls", "sol", "stc", "sts", "txt", "xln",
]);

export const pcbRoleLabels: Readonly<Record<PcbLayerRole, string>> = Object.freeze({
  copper: "Медь",
  drill: "Сверловка",
  outline: "Контур",
  marking: "Маркировка",
  ignore: "Не использовать",
});

export const inferPcbRole = (name: string, content?: Uint8Array | string): PcbLayerRole => {
  if (looksLikeExcellon(content)) return "drill";
  const fileFunction = gerberFileFunction(content);
  if (fileFunction) {
    if (fileFunction === "copper") return "copper";
    if (["plated", "nonplated"].includes(fileFunction)) return "drill";
    if (fileFunction === "profile") return "outline";
    if (fileFunction === "legend") return "marking";
    return "ignore";
  }
  const extension = name.split(".").pop()?.toLowerCase();
  if (["drl", "drd", "exc", "ncd", "xln"].includes(extension ?? "")) return "drill";
  if (["dim", "gko", "gm1", "gm2", "gml", "mil", "oln"].includes(extension ?? "")) return "outline";
  if (["gbo", "gto", "plc", "pls"].includes(extension ?? "")) return "marking";
  if (["gba", "gbp", "gbs", "gta", "gtp", "gts", "stc", "sts"].includes(extension ?? "")) return "ignore";
  return "copper";
};

export const readPcbFiles = async (files: FileList | readonly File[]): Promise<LocalPcbFile[]> => {
  if (files.length > 16) throw new Error("За один раз можно загрузить не более 16 слоёв");
  const decoded = await Promise.all([...files].map(async (file) => ({
    file,
    bytes: new Uint8Array(await file.arrayBuffer()),
  })));
  const accepted = decoded.filter(({ file, bytes }) => {
    const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
    return allowedExtensions.has(extension) || looksLikePcbSource(bytes);
  });
  if (accepted.length === 0) {
    throw new Error("Выберите текстовый Gerber или Excellon. ZIP-архив сначала распакуйте");
  }
  return accepted.map(({ file, bytes }) => ({
    sourceName: file.name,
    sourceBase64: bytesToBase64(bytes),
    role: inferPcbRole(file.name, bytes),
    sizeBytes: file.size,
  }));
};

export const closestTool = (
  tools: readonly CuttingTool[],
  diameterMm: number,
): CuttingTool | undefined => [...tools].sort(
  (left, right) => Math.abs(left.diameterMm - diameterMm) - Math.abs(right.diameterMm - diameterMm),
)[0];

export const isPcbDrillingTool = (tool: CuttingTool): boolean =>
  tool.kind === "drill" || tool.kind === "flatEndMill" || tool.kind === "engraving";

export const isPcbSlotTool = (tool: CuttingTool): boolean =>
  tool.kind === "flatEndMill" || tool.kind === "engraving";

export const toolsForDrillGroup = (
  group: PcbDrillGroup,
  tools: readonly CuttingTool[],
): readonly CuttingTool[] => tools.filter((tool) =>
  isPcbDrillingTool(tool)
  && (group.slotCount === 0 || isPcbSlotTool(tool))
  && tool.diameterMm <= group.diameterMm + 0.01,
);

export const drillMappings = (
  groups: readonly PcbDrillGroup[],
  tools: readonly CuttingTool[],
  current: ReadonlyMap<string, string>,
): Map<string, string> => new Map(groups.map((group) => {
  const compatible = toolsForDrillGroup(group, tools);
  const retained = current.get(group.key);
  const toolId = compatible.some((tool) => tool.id === retained)
    ? retained!
    : closestTool(compatible, group.diameterMm)?.id ?? "";
  return [group.key, toolId];
}));

export const initialPcbOperations = (
  settings: PcbJobSettings,
  files: readonly LocalPcbFile[],
): PcbJobSettings => ({
  ...settings,
  isolation: {
    ...settings.isolation,
    enabled: files.some((file) => file.role === "copper"),
  },
  drilling: {
    ...settings.drilling,
    enabled: files.some((file) => file.role === "drill"),
  },
  outline: { ...settings.outline, enabled: false },
  marking: { ...settings.marking, enabled: false },
});

export const validatePcbWorkflow = (
  files: readonly LocalPcbFile[],
  inspection: PcbInspection | undefined,
  settings: PcbJobSettings,
  tools: readonly CuttingTool[],
): string | undefined => {
  if (files.length === 0) return "Добавьте Gerber или Excellon";
  if (!inspection) return "Дождитесь разбора слоёв";
  if (
    !settings.isolation.enabled
    && !settings.drilling.enabled
    && !settings.outline.enabled
    && !settings.marking.enabled
  ) return "Включите хотя бы одну операцию";

  const toolIds = new Set(tools.map((tool) => tool.id));
  if (settings.isolation.enabled && !toolIds.has(settings.isolation.toolId)) {
    return "Выберите инструмент для изоляции";
  }
  if (settings.drilling.enabled) {
    if (!files.some((file) => file.role === "drill")) {
      return "Для сверловки добавьте Excellon или Gerber X2 drill";
    }
    if (inspection.drillGroups.length === 0) {
      return "В файле сверловки не найдено отверстий или пазов";
    }
    if (inspection.drillGroups.some((group) => {
      const toolId = settings.drilling.mappings.find((mapping) => mapping.groupKey === group.key)?.toolId;
      const mappedTool = tools.find((tool) => tool.id === toolId);
      return !mappedTool || !toolsForDrillGroup(group, [mappedTool]).length;
    })) return "Выберите инструмент для каждой группы отверстий и пазов";
  }
  if (settings.outline.enabled && !toolIds.has(settings.outline.toolId)) {
    return "Выберите инструмент для контура";
  }
  if (settings.marking.enabled && !toolIds.has(settings.marking.toolId)) {
    return "Выберите инструмент для маркировки";
  }
  if (settings.safeZMm <= settings.surfaceZMm) {
    return "Безопасный Z должен быть выше поверхности";
  }
  return undefined;
};

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
};

export const readablePcbError = (reason: unknown): string => {
  const message = reason instanceof Error ? reason.message : String(reason);
  const oversized = /^tool (.+) is wider than PCB drill\/slot group (.+): ([\d.]+) mm > ([\d.]+) mm$/.exec(message);
  if (oversized) {
    return `Инструмент ${oversized[1]} шире отверстия/паза ${oversized[4]} мм`;
  }
  const slotDrill = /^PCB slot group (.+) requires a milling tool, not drill (.+)$/.exec(message);
  if (slotDrill) {
    return `Для паза ${slotDrill[1]} выберите концевую фрезу вместо ${slotDrill[2]}`;
  }
  const unsupportedGerber = /^Gerber file (.+) uses unsupported feature: (.+)$/.exec(message);
  if (unsupportedGerber) {
    return gerberFeatureHelp(unsupportedGerber[1], unsupportedGerber[2]);
  }
  const invalidGerber = /^Gerber file (.+) is invalid: (.+)$/.exec(message);
  if (invalidGerber) {
    return `Не удалось прочитать Gerber «${invalidGerber[1]}»: ${invalidGerber[2]}`;
  }
  const invalidExcellon = /^Excellon file (.+) is invalid: (.+)$/.exec(message);
  if (invalidExcellon) {
    return `Не удалось прочитать файл сверловки «${invalidExcellon[1]}»: ${invalidExcellon[2]}`;
  }
  return message
    .replace("PCB operation requires the copper layer", "Добавьте Gerber-слой меди или выключите изоляцию")
    .replace("PCB operation requires the outline layer", "Добавьте контур платы или выключите вырезание")
    .replace("PCB operation requires the marking layer", "Добавьте слой маркировки или выключите маркировку")
    .replace("PCB drilling requires a tool mapping for each enabled group", "Выберите инструмент для каждой группы отверстий и пазов")
    .replace("PCB job contains no usable copper, drill, outline or marking layers", "Все загруженные слои помечены «Не использовать»");
};

const gerberFeatureHelp = (sourceName: string, feature: string): string => {
  const prefix = `Gerber «${sourceName}»`;
  if (feature === "incremental coordinates") {
    return `${prefix}: экспортируйте координаты в абсолютном режиме`;
  }
  if (feature === "negative file polarity without a finite image boundary") {
    return `${prefix}: отрицательная полярность не задаёт границу изображения; экспортируйте positive polarity`;
  }
  if (feature === "aperture transform LM/LR/LS or aperture block AB") {
    return `${prefix}: повторите экспорт без aperture transforms/blocks`;
  }
  if (feature === "deprecated image transform") {
    return `${prefix}: повторите экспорт без устаревших image transforms`;
  }
  if (feature === "conflicting legacy and extended units") {
    return `${prefix}: в файле одновременно заданы разные единицы; повторите CAM-экспорт`;
  }
  return `${prefix}: неподдерживаемая конструкция «${feature}»`;
};

const gerberFileFunction = (content?: Uint8Array | string): string | undefined => {
  if (!content) return undefined;
  const text = typeof content === "string" ? content : new TextDecoder("ascii").decode(content.subarray(0, 64 * 1024));
  return /(?:TF\.FileFunction|#@!\s*TF\.FileFunction)\s*,\s*([^,*%\r\n]+)/i
    .exec(text)?.[1]?.trim().toLowerCase();
};

const looksLikeExcellon = (content?: Uint8Array | string): boolean => {
  if (!content) return false;
  const text = typeof content === "string" ? content : new TextDecoder("ascii").decode(content.subarray(0, 4096));
  return /(?:^|[\r\n])\s*M48(?:\s|$)/i.test(text.replace(/^\uFEFF/, ""));
};

const looksLikePcbSource = (bytes: Uint8Array): boolean => {
  const text = new TextDecoder("ascii").decode(bytes.subarray(0, 4096)).toUpperCase();
  return (text.includes("%FS") && (text.includes("%MO") || text.includes("%AD")))
    || /(?:^|[\r\n])\s*M48(?:\s|$)/.test(text.replace(/^\uFEFF/, ""));
};
