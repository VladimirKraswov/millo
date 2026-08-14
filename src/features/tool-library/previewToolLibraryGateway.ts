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
    {
      ...tool("preset-dreanique-sp1f-d1-0-l03", "Однозаходная 1 мм · SP1F-D1.0-L03", "flatEndMill", 1, 3.175, undefined, "Полированная однозаходная фреза DreaNique с удалением стружки вверх. Для мелких пазов и контуров; чувствительна к биению, вылету и резкому врезанию."),
      cuttingLengthMm: 3,
      fluteCount: 1,
      feedMmPerMin: 250,
      plungeMmPerMin: 60,
      stepdownMm: 0.2,
      stepoverPercent: 30,
      reference: {
        manufacturer: "DreaNique",
        product: "SP1F-D1.0-L03",
        url: "https://www.dreanique.com/milling-cutter/engraving-and-carving-end-mills/solid-carbide-single-flute-spiral-end-mills.html",
      },
    },
    {
      ...tool("preset-dreanique-sp1f-d2-0-l04", "Однозаходная 2 мм · SP1F-D2.0-L04", "flatEndMill", 2, 3.175, undefined, "Полированная однозаходная фреза DreaNique с удалением стружки вверх. Хорошо выводит стружку, но тонкую заготовку нужно надёжно прижать."),
      cuttingLengthMm: 4,
      fluteCount: 1,
      feedMmPerMin: 400,
      plungeMmPerMin: 100,
      stepdownMm: 0.4,
      reference: {
        manufacturer: "DreaNique",
        product: "SP1F-D2.0-L04",
        url: "https://www.dreanique.com/milling-cutter/engraving-and-carving-end-mills/solid-carbide-single-flute-spiral-end-mills.html",
      },
    },
    {
      ...tool("preset-downcut-3-175-2-17", "Однозаходная 2 мм, стружка вниз · 3,175×2×17", "flatEndMill", 2, 3.175, undefined, "Однозаходная downcut-фреза уменьшает сколы верхней кромки. Стружка остаётся в пазу, поэтому нужны неглубокие проходы и регулярная очистка."),
      cuttingLengthMm: 17,
      fluteCount: 1,
      feedMmPerMin: 350,
      plungeMmPerMin: 80,
      stepdownMm: 0.4,
      reference: {
        manufacturer: "Без маркировки производителя",
        product: "Downcut 3,175×2×17",
        url: "https://www.walmart.com/ip/3-175mm-Milling-Cutter-Left-hand-CNC-Carbide-End-Mill-Spiral-Woodworking-Tool-For-Power-Tools-Drill-Bits-Accessory/16606166421",
      },
    },
    {
      ...tool("preset-cerin-64l-060a", "Cerin 64L.060A · концевая 4-зубая 6 мм", "flatEndMill", 6, 6, undefined, "Длинная покрытая цельнотвердосплавная четырёхзубая фреза Cerin 6×30×70 мм. Серия предназначена для сталей, нержавеющих сталей, чугуна и металлов средней твёрдости до 55 HRC."),
      cuttingLengthMm: 30,
      fluteCount: 4,
      feedMmPerMin: 300,
      plungeMmPerMin: 60,
      spindleRpm: 12_000,
      stepdownMm: 0.3,
      stepoverPercent: 25,
      reference: {
        manufacturer: "Cerin",
        product: "64L.060A",
        url: "https://www.cerin.it/frese/fresatura-acciaio-e-metalli-ferrosi/fresa-standard-a-4-taglienti-lunga",
      },
    },
    tool("preset-202", "Шаровая 6,35 мм · #202", "ballNose", 6.35, 6.35, undefined, "Шаровая фреза для чистовых проходов по рельефам и плавным 3D-поверхностям."),
    tool("preset-302", "V-фреза 60° · #302", "vBit", 12.7, 6.35, 60, "V-фреза 60° формирует узкие линии и сохраняет мелкие детали в надписях и декоративной гравировке."),
    tool("preset-301", "V-фреза 90° · #301", "vBit", 12.7, 6.35, 90, "V-фреза 90° подходит для более широких надписей, фасок и декоративных канавок."),
    tool("preset-501", "Гравёр 60° · #501", "engraving", 2.54, 3.175, 60, "Остроконечный гравёр для мелкой маркировки, тонких линий и PCB."),
    {
      ...tool("preset-xc-nlj3-2001", "Гравёр 20° × 0,1 мм · XC-NLJ3.2001", "engraving", 0.1, 3.175, 20, "Точный V-гравёр с кончиком 0,1 мм и общей длиной 40 мм. Пресет ограничивает не указанную на футляре рабочую длину консервативными 3 мм."),
      cuttingLengthMm: 3,
      fluteCount: 1,
      feedMmPerMin: 120,
      plungeMmPerMin: 40,
      stepdownMm: 0.05,
      stepoverPercent: 10,
      reference: {
        manufacturer: "XC",
        product: "XC-NLJ3.2001",
        url: "https://www.didacticaselectronicas.com/shop/xc-nlj3-2001-broca-para-grabado-de-3-175mm-vastago-en-forma-de-v-20-grados-23631",
      },
    },
    {
      ...tool("preset-v-engraver-90-0-1", "V-гравёр 90° × 0,1 мм", "engraving", 0.1, 3.175, 90, "V-гравёр для широких неглубоких канавок, надписей и фасок. Перед работой проверьте кромки и рабочую длину: на футляре указан только угол и кончик."),
      cuttingLengthMm: 14,
      feedMmPerMin: 180,
      plungeMmPerMin: 50,
      stepdownMm: 0.1,
      stepoverPercent: 10,
      reference: {
        manufacturer: "Без маркировки производителя",
        product: "V-гравёр 90° × 0,1 мм",
        url: "https://www.harfington.com/products/p-1869351",
      },
    },
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
