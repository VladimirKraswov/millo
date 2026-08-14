import type { ToolLibraryGateway } from "../../platform/tooling/ToolLibraryGateway";
import type { CuttingTool, ToolKind, ToolLibraryState } from "../../shared/tooling";

const tool = (
  id: string,
  name: string,
  kind: ToolKind,
  diameterMm: number,
  shankDiameterMm: number,
  includedAngleDegrees?: number,
  description = "Пользовательский инструмент. Перед работой уточните режимы по паспорту производителя.",
): CuttingTool => ({
  id,
  name,
  description,
  kind,
  diameterMm,
  shankDiameterMm,
  cuttingLengthMm: kind === "surfacing" ? 10 : 19.05,
  fluteCount: kind === "surfacing" ? 4 : 2,
  includedAngleDegrees,
  feedMmPerMin: kind === "surfacing" ? 1_524 : 600,
  plungeMmPerMin: kind === "surfacing" ? 300 : 180,
  spindleRpm: kind === "surfacing" ? 16_000 : 18_000,
  stepdownMm: kind === "surfacing" ? 0.508 : 1,
  stepoverPercent: kind === "surfacing" ? 45 : 35,
  factoryPreset: true,
  reference: {
    manufacturer: "Carbide 3D",
    product: name,
    url: kind === "surfacing"
      ? "https://shop.carbide3d.com/products/mcflycutter"
      : "https://shop.carbide3d.com/collections/carbide-3d-cutters",
  },
});

let snapshot: ToolLibraryState = {
  revision: 0,
  tools: [
    tool("preset-102", "Плоская 3,175 мм · #102", "flatEndMill", 3.175, 3.175, undefined, "Компактная плоская фреза общего назначения для небольших пазов, карманов, контуров и черновой обработки."),
    tool("preset-201", "Плоская 6,35 мм · #201", "flatEndMill", 6.35, 6.35, undefined, "Жёсткая плоская фреза для быстрого снятия материала, крупных пазов, карманов и раскроя."),
    {
      ...tool("preset-inreko-cct01-2f-06050-06", "Концевая твердосплавная 6 мм · CCT01-2F-06050.06", "flatEndMill", 6, 6, undefined, "Двухзубая плоская цельнотвердосплавная фреза 6×15×50 мм для пазов, карманов, контуров и выборки материала. Подбирайте режимы под конкретный материал."),
      cuttingLengthMm: 15,
      fluteCount: 2,
      feedMmPerMin: 450,
      plungeMmPerMin: 100,
      spindleRpm: 12_000,
      stepdownMm: 0.5,
      stepoverPercent: 30,
      reference: {
        manufacturer: "ИНРЕКО",
        product: "CCT01-2F-06050.06 6×15×50",
        url: "https://inreko.ru/katalog/",
      },
    },
    tool("preset-202", "Шаровая 6,35 мм · #202", "ballNose", 6.35, 6.35, undefined, "Шаровая фреза для чистовых проходов по рельефам и плавным 3D-поверхностям."),
    tool("preset-302", "V-фреза 60° · #302", "vBit", 12.7, 6.35, 60, "V-фреза 60° формирует узкие линии и сохраняет мелкие детали в надписях и декоративной гравировке."),
    tool("preset-301", "V-фреза 90° · #301", "vBit", 12.7, 6.35, 90, "V-фреза 90° подходит для более широких надписей, фасок и декоративных канавок."),
    tool("preset-501", "Гравёр 60° · #501", "engraving", 2.54, 3.175, 60, "Остроконечный гравёр для мелкой маркировки, тонких линий и PCB."),
    tool("preset-mcfly", "Торцевая 25,4 мм · McFly", "surfacing", 25.4, 6.35, undefined, "Широкая сменнопластинчатая фреза для выравнивания жертвенного стола и деревянных плит."),
  ],
};

export const previewToolLibraryGateway: ToolLibraryGateway = {
  load: async () => snapshot,
  create: async (draft) => {
    snapshot = {
      revision: snapshot.revision + 1,
      tools: [...snapshot.tools, {
        id: `preview-${snapshot.revision + 1}`,
        ...draft,
        factoryPreset: false,
      }],
    };
    return snapshot;
  },
  update: async (toolId, draft) => {
    snapshot = {
      revision: snapshot.revision + 1,
      tools: snapshot.tools.map((item) => item.id === toolId
        ? { ...item, ...draft }
        : item),
    };
    return snapshot;
  },
  delete: async (toolId) => {
    snapshot = {
      revision: snapshot.revision + 1,
      tools: snapshot.tools.filter((item) => item.id !== toolId),
    };
    return snapshot;
  },
  restorePresets: async () => snapshot,
};
