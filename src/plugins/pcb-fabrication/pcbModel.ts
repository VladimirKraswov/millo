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
  "art", "drl", "gbl", "gbo", "gbr", "gko", "gm1", "gml", "gto", "gtl", "txt", "xln",
]);

export const pcbRoleLabels: Readonly<Record<PcbLayerRole, string>> = Object.freeze({
  copper: "Медь",
  drill: "Сверловка",
  outline: "Контур",
  marking: "Маркировка",
});

export const inferPcbRole = (name: string): PcbLayerRole => {
  const extension = name.split(".").pop()?.toLowerCase();
  if (["drl", "xln", "txt"].includes(extension ?? "")) return "drill";
  if (["gko", "gm1", "gml"].includes(extension ?? "")) return "outline";
  if (["gto", "gbo"].includes(extension ?? "")) return "marking";
  return "copper";
};

export const readPcbFiles = async (files: FileList | readonly File[]): Promise<LocalPcbFile[]> => {
  const accepted = [...files].filter((file) => allowedExtensions.has(file.name.split(".").pop()?.toLowerCase() ?? ""));
  if (accepted.length === 0) throw new Error("Выберите Gerber или Excellon: .gbr, .gtl, .gko, .drl");
  if (accepted.length > 16) throw new Error("За один раз можно загрузить не более 16 слоёв");
  return Promise.all(accepted.map(async (file) => ({
    sourceName: file.name,
    sourceBase64: bytesToBase64(new Uint8Array(await file.arrayBuffer())),
    role: inferPcbRole(file.name),
    sizeBytes: file.size,
  })));
};

export const closestTool = (
  tools: readonly CuttingTool[],
  diameterMm: number,
): CuttingTool | undefined => [...tools].sort(
  (left, right) => Math.abs(left.diameterMm - diameterMm) - Math.abs(right.diameterMm - diameterMm),
)[0];

export const isPcbDrillingTool = (tool: CuttingTool): boolean =>
  tool.kind === "drill" || tool.kind === "flatEndMill" || tool.kind === "engraving";

export const drillMappings = (
  groups: readonly PcbDrillGroup[],
  tools: readonly CuttingTool[],
  current: ReadonlyMap<string, string>,
): Map<string, string> => new Map(groups.map((group) => {
  const retained = current.get(group.key);
  const toolId = tools.some((tool) => tool.id === retained)
    ? retained!
    : closestTool(tools, group.diameterMm)?.id ?? "";
  return [group.key, toolId];
}));

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
      return "Для сверловки добавьте Excellon (.drl, .xln или .txt)";
    }
    if (inspection.drillGroups.length === 0) {
      return "В Excellon не найдено ни одной группы отверстий";
    }
    if (inspection.drillGroups.some((group) => {
      const toolId = settings.drilling.mappings.find((mapping) => mapping.groupKey === group.key)?.toolId;
      const mappedTool = tools.find((tool) => tool.id === toolId);
      return !mappedTool || !isPcbDrillingTool(mappedTool);
    })) return "Выберите сверло для каждой группы отверстий";
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
  return message
    .replace("PCB operation requires the copper layer", "Добавьте Gerber-слой меди или выключите изоляцию")
    .replace("PCB operation requires the outline layer", "Добавьте контур платы или выключите вырезание")
    .replace("PCB operation requires the marking layer", "Добавьте слой маркировки или выключите маркировку")
    .replace("PCB drilling requires a tool mapping for each enabled group", "Выберите сверло для каждой группы отверстий");
};
