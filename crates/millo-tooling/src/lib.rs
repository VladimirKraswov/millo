use std::{
    fs, io,
    path::{Path, PathBuf},
};

use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u16 = 1;
const PRESET_CATALOG_VERSION: u16 = 1;
const CCT01_2F_06050_PRESET_ID: &str = "preset-inreko-cct01-2f-06050-06";
const MAX_TOOLS: usize = 256;
const MAX_NAME_BYTES: usize = 100;
const MAX_DESCRIPTION_BYTES: usize = 2_000;
const MAX_REFERENCE_BYTES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolKind {
    FlatEndMill,
    BallNose,
    VBit,
    Engraving,
    Surfacing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolReference {
    pub manufacturer: String,
    pub product: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuttingTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: ToolKind,
    pub diameter_mm: f64,
    pub shank_diameter_mm: f64,
    pub cutting_length_mm: f64,
    pub flute_count: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_angle_degrees: Option<f64>,
    pub feed_mm_per_min: f64,
    pub plunge_mm_per_min: f64,
    pub spindle_rpm: u32,
    pub stepdown_mm: f64,
    pub stepover_percent: f64,
    pub factory_preset: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<ToolReference>,
}

impl CuttingTool {
    pub fn supports_surfacing(&self) -> bool {
        matches!(self.kind, ToolKind::FlatEndMill | ToolKind::Surfacing)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuttingToolDraft {
    pub name: String,
    pub description: String,
    pub kind: ToolKind,
    pub diameter_mm: f64,
    pub shank_diameter_mm: f64,
    pub cutting_length_mm: f64,
    pub flute_count: u8,
    pub included_angle_degrees: Option<f64>,
    pub feed_mm_per_min: f64,
    pub plunge_mm_per_min: f64,
    pub spindle_rpm: u32,
    pub stepdown_mm: f64,
    pub stepover_percent: f64,
}

impl From<&CuttingTool> for CuttingToolDraft {
    fn from(tool: &CuttingTool) -> Self {
        Self {
            name: tool.name.clone(),
            description: tool.description.clone(),
            kind: tool.kind,
            diameter_mm: tool.diameter_mm,
            shank_diameter_mm: tool.shank_diameter_mm,
            cutting_length_mm: tool.cutting_length_mm,
            flute_count: tool.flute_count,
            included_angle_degrees: tool.included_angle_degrees,
            feed_mm_per_min: tool.feed_mm_per_min,
            plunge_mm_per_min: tool.plunge_mm_per_min,
            spindle_rpm: tool.spindle_rpm,
            stepdown_mm: tool.stepdown_mm,
            stepover_percent: tool.stepover_percent,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolLibraryState {
    pub tools: Vec<CuttingTool>,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredToolLibrary {
    schema_version: u16,
    #[serde(default)]
    preset_catalog_version: u16,
    next_id: u64,
    tools: Vec<CuttingTool>,
    revision: u64,
}

impl Default for StoredToolLibrary {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            preset_catalog_version: PRESET_CATALOG_VERSION,
            next_id: 1,
            tools: factory_presets(),
            revision: 0,
        }
    }
}

#[derive(Debug)]
pub struct ToolLibraryStore {
    path: Option<PathBuf>,
    document: StoredToolLibrary,
}

impl ToolLibraryStore {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            document: StoredToolLibrary::default(),
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, ToolLibraryError> {
        let path = path.into();
        let backup = backup_path(&path);
        if !path.exists() && !backup.exists() {
            return Ok(Self {
                path: Some(path),
                document: StoredToolLibrary::default(),
            });
        }
        let (mut document, recovered) = if path.exists() {
            match load_document(&path) {
                Ok(document) => (document, false),
                Err(ToolLibraryError::InvalidFile(primary)) if backup.exists() => {
                    match load_document(&backup) {
                        Ok(document) => (document, true),
                        Err(ToolLibraryError::InvalidFile(backup)) => {
                            return Err(ToolLibraryError::CorruptCopies { primary, backup });
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            }
        } else {
            (load_document(&backup)?, true)
        };
        let migrated = migrate_preset_catalog(&mut document);
        let refreshed = refresh_unedited_preset_descriptions(&mut document);
        if recovered && path.exists() {
            fs::remove_file(&path).map_err(|source| ToolLibraryError::Io {
                path: path.clone(),
                source,
            })?;
        }
        if recovered || migrated || refreshed {
            validate_document(&document)
                .map_err(|error| ToolLibraryError::InvalidFile(error.to_string()))?;
            save_document(&path, &document)?;
        }
        Ok(Self {
            path: Some(path),
            document,
        })
    }

    pub fn state(&self) -> ToolLibraryState {
        ToolLibraryState {
            tools: self.document.tools.clone(),
            revision: self.document.revision,
        }
    }

    pub fn get(&self, id: &str) -> Option<&CuttingTool> {
        self.document.tools.iter().find(|tool| tool.id == id)
    }

    pub fn create(
        &mut self,
        draft: CuttingToolDraft,
    ) -> Result<ToolLibraryState, ToolLibraryError> {
        validate_draft(&draft)?;
        self.ensure_name_available(&draft.name, None)?;
        if self.document.tools.len() >= MAX_TOOLS {
            return Err(ToolLibraryError::ToolLimit(MAX_TOOLS));
        }
        let mut next = self.document.clone();
        let tool = tool_from_draft(format!("tool-{:04}", next.next_id), draft, false, None);
        next.next_id = next
            .next_id
            .checked_add(1)
            .ok_or(ToolLibraryError::IdExhausted)?;
        next.tools.push(tool);
        self.commit(next)?;
        Ok(self.state())
    }

    pub fn update(
        &mut self,
        id: &str,
        draft: CuttingToolDraft,
    ) -> Result<ToolLibraryState, ToolLibraryError> {
        validate_draft(&draft)?;
        self.ensure_name_available(&draft.name, Some(id))?;
        let mut next = self.document.clone();
        let tool = next
            .tools
            .iter_mut()
            .find(|tool| tool.id == id)
            .ok_or_else(|| ToolLibraryError::UnknownTool(id.to_owned()))?;
        let factory_preset = tool.factory_preset;
        let reference = tool.reference.clone();
        *tool = tool_from_draft(id.to_owned(), draft, factory_preset, reference);
        self.commit(next)?;
        Ok(self.state())
    }

    pub fn delete(&mut self, id: &str) -> Result<ToolLibraryState, ToolLibraryError> {
        let mut next = self.document.clone();
        let original_len = next.tools.len();
        next.tools.retain(|tool| tool.id != id);
        if next.tools.len() == original_len {
            return Err(ToolLibraryError::UnknownTool(id.to_owned()));
        }
        self.commit(next)?;
        Ok(self.state())
    }

    pub fn restore_missing_presets(&mut self) -> Result<ToolLibraryState, ToolLibraryError> {
        let mut next = self.document.clone();
        for preset in factory_presets() {
            if !next.tools.iter().any(|tool| tool.id == preset.id) {
                if next.tools.len() >= MAX_TOOLS {
                    return Err(ToolLibraryError::ToolLimit(MAX_TOOLS));
                }
                next.tools.push(preset);
            }
        }
        if next.tools == self.document.tools {
            return Ok(self.state());
        }
        self.commit(next)?;
        Ok(self.state())
    }

    fn ensure_name_available(
        &self,
        name: &str,
        except_id: Option<&str>,
    ) -> Result<(), ToolLibraryError> {
        let name = name.trim();
        if self
            .document
            .tools
            .iter()
            .any(|tool| Some(tool.id.as_str()) != except_id && tool.name.eq_ignore_ascii_case(name))
        {
            return Err(ToolLibraryError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }

    fn commit(&mut self, mut next: StoredToolLibrary) -> Result<(), ToolLibraryError> {
        next.revision = self.document.revision.saturating_add(1);
        validate_document(&next)?;
        if let Some(path) = self.path.as_deref() {
            save_document(path, &next)?;
        }
        self.document = next;
        Ok(())
    }
}

fn tool_from_draft(
    id: String,
    draft: CuttingToolDraft,
    factory_preset: bool,
    reference: Option<ToolReference>,
) -> CuttingTool {
    CuttingTool {
        id,
        name: draft.name.trim().to_owned(),
        description: draft.description.trim().to_owned(),
        kind: draft.kind,
        diameter_mm: draft.diameter_mm,
        shank_diameter_mm: draft.shank_diameter_mm,
        cutting_length_mm: draft.cutting_length_mm,
        flute_count: draft.flute_count,
        included_angle_degrees: draft.included_angle_degrees,
        feed_mm_per_min: draft.feed_mm_per_min,
        plunge_mm_per_min: draft.plunge_mm_per_min,
        spindle_rpm: draft.spindle_rpm,
        stepdown_mm: draft.stepdown_mm,
        stepover_percent: draft.stepover_percent,
        factory_preset,
        reference,
    }
}

struct FactoryToolSpec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    kind: ToolKind,
    diameter_mm: f64,
    shank_diameter_mm: f64,
    cutting_length_mm: f64,
    flute_count: u8,
    included_angle_degrees: Option<f64>,
    feed_mm_per_min: f64,
    plunge_mm_per_min: f64,
    spindle_rpm: u32,
    stepdown_mm: f64,
    stepover_percent: f64,
    reference: ToolReference,
}

fn factory_tool(spec: FactoryToolSpec) -> CuttingTool {
    CuttingTool {
        id: spec.id.to_owned(),
        name: spec.name.to_owned(),
        description: spec.description.to_owned(),
        kind: spec.kind,
        diameter_mm: spec.diameter_mm,
        shank_diameter_mm: spec.shank_diameter_mm,
        cutting_length_mm: spec.cutting_length_mm,
        flute_count: spec.flute_count,
        included_angle_degrees: spec.included_angle_degrees,
        feed_mm_per_min: spec.feed_mm_per_min,
        plunge_mm_per_min: spec.plunge_mm_per_min,
        spindle_rpm: spec.spindle_rpm,
        stepdown_mm: spec.stepdown_mm,
        stepover_percent: spec.stepover_percent,
        factory_preset: true,
        reference: Some(spec.reference),
    }
}

fn carbide_reference(product: &str, url: &str) -> ToolReference {
    ToolReference {
        manufacturer: "Carbide 3D".to_owned(),
        product: product.to_owned(),
        url: url.to_owned(),
    }
}

pub fn default_description(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::FlatEndMill => {
            "Плоский торец формирует ровное дно. Подходит для выборок, контуров, пазов и черновой обработки."
        }
        ToolKind::BallNose => {
            "Сферический торец предназначен для чистовой обработки рельефов и плавных 3D-поверхностей."
        }
        ToolKind::VBit => {
            "V-образная геометрия меняет ширину реза вместе с глубиной и подходит для надписей, фасок и V-carving."
        }
        ToolKind::Engraving => {
            "Тонкий гравировальный кончик создаёт мелкие линии, маркировку и дорожки при небольшой глубине."
        }
        ToolKind::Surfacing => {
            "Широкая торцевая фреза снимает тонкий равномерный слой с жертвенного стола или деревянной панели."
        }
    }
}

pub fn factory_presets() -> Vec<CuttingTool> {
    vec![
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-102",
            name: "Плоская 3,175 мм · #102",
            description: "Компактная плоская фреза общего назначения. Удобна для небольших пазов, карманов, контуров и черновой обработки, когда фреза 6,35 мм уже слишком крупная.",
            kind: ToolKind::FlatEndMill,
            diameter_mm: 3.175,
            shank_diameter_mm: 3.175,
            cutting_length_mm: 19.05,
            flute_count: 2,
            included_angle_degrees: None,
            feed_mm_per_min: 600.0,
            plunge_mm_per_min: 180.0,
            spindle_rpm: 18_000,
            stepdown_mm: 1.0,
            stepover_percent: 40.0,
            reference: carbide_reference(
                "#102 .125 in Flat Cutter",
                "https://shop.carbide3d.com/products/102-125-end-mill-cutter",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-201",
            name: "Плоская 6,35 мм · #201",
            description: "Более жёсткая плоская фреза для быстрого снятия материала, крупных пазов, карманов и раскроя. Требует достаточной мощности шпинделя и надёжного закрепления заготовки.",
            kind: ToolKind::FlatEndMill,
            diameter_mm: 6.35,
            shank_diameter_mm: 6.35,
            cutting_length_mm: 19.05,
            flute_count: 3,
            included_angle_degrees: None,
            feed_mm_per_min: 900.0,
            plunge_mm_per_min: 250.0,
            spindle_rpm: 18_000,
            stepdown_mm: 1.5,
            stepover_percent: 40.0,
            reference: carbide_reference(
                "#201 .25 in Flat Cutter",
                "https://shop.carbide3d.com/products/201-25-end-mill-cutter",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: CCT01_2F_06050_PRESET_ID,
            name: "Концевая твердосплавная 6 мм · CCT01-2F-06050.06",
            description: "Двухзубая плоская цельнотвердосплавная фреза 6×15×50 мм для пазов, карманов, контуров и выборки материала. Две широкие канавки помогают отводить стружку. Подбирайте подачу и обороты под конкретный материал: режимы для дерева, алюминия и стали нельзя переносить без проверки.",
            kind: ToolKind::FlatEndMill,
            diameter_mm: 6.0,
            shank_diameter_mm: 6.0,
            cutting_length_mm: 15.0,
            flute_count: 2,
            included_angle_degrees: None,
            feed_mm_per_min: 450.0,
            plunge_mm_per_min: 100.0,
            spindle_rpm: 12_000,
            stepdown_mm: 0.5,
            stepover_percent: 30.0,
            reference: ToolReference {
                manufacturer: "ИНРЕКО".to_owned(),
                product: "CCT01-2F-06050.06 6×15×50".to_owned(),
                url: "https://inreko.ru/katalog/".to_owned(),
            },
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-202",
            name: "Шаровая 6,35 мм · #202",
            description: "Шаровая фреза для чистовых проходов по рельефам и плавным 3D-поверхностям. Обычно применяется после черновой выборки плоской фрезой с небольшим поперечным шагом.",
            kind: ToolKind::BallNose,
            diameter_mm: 6.35,
            shank_diameter_mm: 6.35,
            cutting_length_mm: 19.05,
            flute_count: 3,
            included_angle_degrees: None,
            feed_mm_per_min: 750.0,
            plunge_mm_per_min: 200.0,
            spindle_rpm: 18_000,
            stepdown_mm: 1.0,
            stepover_percent: 20.0,
            reference: carbide_reference(
                "#202 .25 in Ball Cutter",
                "https://shop.carbide3d.com/products/202-25-ball-cutter",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-302",
            name: "V-фреза 60° · #302",
            description: "V-фреза 60° формирует узкие линии и лучше сохраняет мелкие детали в надписях и декоративной гравировке. Итоговая ширина линии особенно чувствительна к глубине и нулю Z.",
            kind: ToolKind::VBit,
            diameter_mm: 12.7,
            shank_diameter_mm: 6.35,
            cutting_length_mm: 6.35,
            flute_count: 2,
            included_angle_degrees: Some(60.0),
            feed_mm_per_min: 600.0,
            plunge_mm_per_min: 150.0,
            spindle_rpm: 18_000,
            stepdown_mm: 0.5,
            stepover_percent: 20.0,
            reference: carbide_reference(
                "#302 0.50 in V-Bit Cutter 60 degrees",
                "https://shop.carbide3d.com/products/302-0-50-v-bit-cutter-60-qty-2",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-301",
            name: "V-фреза 90° · #301",
            description: "V-фреза 90° подходит для более широких надписей, фасок и декоративных канавок. На той же глубине оставляет линию шире, чем 60-градусная фреза.",
            kind: ToolKind::VBit,
            diameter_mm: 12.7,
            shank_diameter_mm: 6.35,
            cutting_length_mm: 6.35,
            flute_count: 2,
            included_angle_degrees: Some(90.0),
            feed_mm_per_min: 600.0,
            plunge_mm_per_min: 150.0,
            spindle_rpm: 18_000,
            stepdown_mm: 0.5,
            stepover_percent: 20.0,
            reference: carbide_reference(
                "#301 0.50 in V-Bit Cutter 90 degrees",
                "https://shop.carbide3d.com/products/301-v-bit-cutter-90",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-501",
            name: "Гравёр 60° · #501",
            description: "Остроконечный гравёр для мелкой маркировки, тонких линий и PCB. Используйте очень небольшой съём, точный рабочий ноль Z и минимальное биение шпинделя.",
            kind: ToolKind::Engraving,
            diameter_mm: 2.54,
            shank_diameter_mm: 3.175,
            cutting_length_mm: 6.35,
            flute_count: 2,
            included_angle_degrees: Some(60.0),
            feed_mm_per_min: 300.0,
            plunge_mm_per_min: 100.0,
            spindle_rpm: 18_000,
            stepdown_mm: 0.2,
            stepover_percent: 12.0,
            reference: carbide_reference(
                "#501 PCB Engraver",
                "https://shop.carbide3d.com/products/501-engraving-bit",
            ),
        }),
        factory_tool(FactoryToolSpec {
            id: "preset-carbide3d-mcfly",
            name: "Торцевая 25,4 мм · McFly",
            description: "Широкая сменнопластинчатая фреза для выравнивания жертвенного стола и деревянных плит. Производитель указывает её для дерева и рекомендует неглубокий проход.",
            kind: ToolKind::Surfacing,
            diameter_mm: 25.4,
            shank_diameter_mm: 6.35,
            cutting_length_mm: 10.0,
            flute_count: 4,
            included_angle_degrees: None,
            feed_mm_per_min: 1_524.0,
            plunge_mm_per_min: 300.0,
            spindle_rpm: 16_000,
            stepdown_mm: 0.508,
            stepover_percent: 45.0,
            reference: carbide_reference(
                "McFly Surfacing Cutter 1/4 in Shank",
                "https://shop.carbide3d.com/products/mcflycutter",
            ),
        }),
    ]
}

fn migrate_preset_catalog(document: &mut StoredToolLibrary) -> bool {
    if document.preset_catalog_version >= PRESET_CATALOG_VERSION {
        return false;
    }

    if document.preset_catalog_version < 1 {
        let preset = factory_presets()
            .into_iter()
            .find(|tool| tool.id == CCT01_2F_06050_PRESET_ID)
            .expect("introduced factory preset must exist");
        let already_present = document
            .tools
            .iter()
            .any(|tool| tool.id == preset.id || tool.name.eq_ignore_ascii_case(&preset.name));
        if document.tools.len() < MAX_TOOLS && !already_present {
            document.tools.push(preset);
        }
    }

    document.preset_catalog_version = PRESET_CATALOG_VERSION;
    document.revision = document.revision.saturating_add(1);
    true
}

fn refresh_unedited_preset_descriptions(document: &mut StoredToolLibrary) -> bool {
    let presets = factory_presets()
        .into_iter()
        .map(|tool| (tool.id.clone(), tool))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut changed = false;
    for tool in &mut document.tools {
        let Some(preset) = presets.get(&tool.id) else {
            continue;
        };
        if tool.factory_preset && tool.description == default_description(tool.kind) {
            tool.description.clone_from(&preset.description);
            changed = true;
        }
    }
    changed
}

fn validate_document(document: &StoredToolLibrary) -> Result<(), ToolLibraryError> {
    if document.schema_version != SCHEMA_VERSION {
        return Err(ToolLibraryError::UnsupportedSchema(document.schema_version));
    }
    if document.preset_catalog_version > PRESET_CATALOG_VERSION {
        return Err(ToolLibraryError::UnsupportedPresetCatalog(
            document.preset_catalog_version,
        ));
    }
    if document.tools.len() > MAX_TOOLS {
        return Err(ToolLibraryError::ToolLimit(MAX_TOOLS));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for tool in &document.tools {
        if tool.id.trim().is_empty() || !ids.insert(tool.id.clone()) {
            return Err(ToolLibraryError::InvalidId(tool.id.clone()));
        }
        let key = tool.name.to_lowercase();
        if !names.insert(key) {
            return Err(ToolLibraryError::DuplicateName(tool.name.clone()));
        }
        validate_draft(&CuttingToolDraft::from(tool))?;
        if let Some(reference) = &tool.reference {
            for value in [&reference.manufacturer, &reference.product, &reference.url] {
                if value.trim().is_empty() || value.len() > MAX_REFERENCE_BYTES {
                    return Err(ToolLibraryError::InvalidReference);
                }
            }
            if !reference.url.starts_with("https://") {
                return Err(ToolLibraryError::InvalidReference);
            }
        }
    }
    Ok(())
}

fn validate_draft(draft: &CuttingToolDraft) -> Result<(), ToolLibraryError> {
    let name = draft.name.trim();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(ToolLibraryError::InvalidName);
    }
    let description = draft.description.trim();
    if description.is_empty() || description.len() > MAX_DESCRIPTION_BYTES {
        return Err(ToolLibraryError::InvalidDescription);
    }
    validate_range("diameterMm", draft.diameter_mm, 0.01, 500.0)?;
    validate_range("shankDiameterMm", draft.shank_diameter_mm, 0.1, 100.0)?;
    validate_range("cuttingLengthMm", draft.cutting_length_mm, 0.1, 1_000.0)?;
    if !(1..=20).contains(&draft.flute_count) {
        return Err(ToolLibraryError::InvalidValue {
            field: "fluteCount",
            value: f64::from(draft.flute_count),
        });
    }
    if matches!(draft.kind, ToolKind::VBit | ToolKind::Engraving) {
        validate_range(
            "includedAngleDegrees",
            draft.included_angle_degrees.unwrap_or(f64::NAN),
            1.0,
            179.0,
        )?;
    } else if draft.included_angle_degrees.is_some() {
        return Err(ToolLibraryError::UnexpectedAngle);
    }
    validate_range("feedMmPerMin", draft.feed_mm_per_min, 1.0, 100_000.0)?;
    validate_range("plungeMmPerMin", draft.plunge_mm_per_min, 1.0, 50_000.0)?;
    if !(1_000..=100_000).contains(&draft.spindle_rpm) {
        return Err(ToolLibraryError::InvalidValue {
            field: "spindleRpm",
            value: f64::from(draft.spindle_rpm),
        });
    }
    validate_range(
        "stepdownMm",
        draft.stepdown_mm,
        0.001,
        draft.cutting_length_mm,
    )?;
    validate_range("stepoverPercent", draft.stepover_percent, 1.0, 95.0)?;
    Ok(())
}

fn validate_range(
    field: &'static str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), ToolLibraryError> {
    if !value.is_finite() || value < min || value > max {
        return Err(ToolLibraryError::InvalidValue { field, value });
    }
    Ok(())
}

fn load_document(path: &Path) -> Result<StoredToolLibrary, ToolLibraryError> {
    let bytes = fs::read(path).map_err(|source| ToolLibraryError::Io {
        path: path.to_owned(),
        source,
    })?;
    let document: StoredToolLibrary = serde_json::from_slice(&bytes)
        .map_err(|error| ToolLibraryError::InvalidFile(error.to_string()))?;
    validate_document(&document)
        .map_err(|error| ToolLibraryError::InvalidFile(error.to_string()))?;
    Ok(document)
}

fn save_document(path: &Path, document: &StoredToolLibrary) -> Result<(), ToolLibraryError> {
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| ToolLibraryError::Serialization(error.to_string()))?;
    write_atomically(path, &bytes).map_err(|error| ToolLibraryError::Storage(error.to_string()))
}

#[derive(Debug, Error)]
pub enum ToolLibraryError {
    #[error("tool library has unsupported schema version {0}")]
    UnsupportedSchema(u16),
    #[error("tool library has unsupported preset catalog version {0}")]
    UnsupportedPresetCatalog(u16),
    #[error("tool library contains invalid id: {0}")]
    InvalidId(String),
    #[error("tool name must contain 1 to {MAX_NAME_BYTES} bytes")]
    InvalidName,
    #[error("tool name already exists: {0}")]
    DuplicateName(String),
    #[error("tool description must contain 1 to {MAX_DESCRIPTION_BYTES} bytes")]
    InvalidDescription,
    #[error("tool reference is invalid")]
    InvalidReference,
    #[error("included angle is valid only for V-bit and engraving tools")]
    UnexpectedAngle,
    #[error("invalid {field}: {value}")]
    InvalidValue { field: &'static str, value: f64 },
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("tool library is limited to {0} entries")]
    ToolLimit(usize),
    #[error("tool id sequence is exhausted")]
    IdExhausted,
    #[error("tool library file is invalid: {0}")]
    InvalidFile(String),
    #[error("both tool library copies are corrupt: primary={primary}; backup={backup}")]
    CorruptCopies { primary: String, backup: String },
    #[error("failed to serialize tool library: {0}")]
    Serialization(String),
    #[error("tool library storage failed: {0}")]
    Storage(String),
    #[error("tool library I/O failed at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn custom(name: &str) -> CuttingToolDraft {
        CuttingToolDraft {
            name: name.to_owned(),
            description: default_description(ToolKind::FlatEndMill).to_owned(),
            kind: ToolKind::FlatEndMill,
            diameter_mm: 3.0,
            shank_diameter_mm: 3.0,
            cutting_length_mm: 12.0,
            flute_count: 2,
            included_angle_degrees: None,
            feed_mm_per_min: 400.0,
            plunge_mm_per_min: 120.0,
            spindle_rpm: 16_000,
            stepdown_mm: 0.8,
            stepover_percent: 35.0,
        }
    }

    fn test_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("millo-tooling-{label}-{nonce}.json"))
    }

    #[test]
    fn starts_with_valid_editable_factory_presets() {
        let mut store = ToolLibraryStore::in_memory();
        let initial = store.state();
        assert_eq!(initial.tools.len(), 8);
        assert!(initial.tools.iter().all(|tool| tool.factory_preset));
        let requested = initial
            .tools
            .iter()
            .find(|tool| tool.id == CCT01_2F_06050_PRESET_ID)
            .unwrap();
        assert_eq!(requested.diameter_mm, 6.0);
        assert_eq!(requested.shank_diameter_mm, 6.0);
        assert_eq!(requested.cutting_length_mm, 15.0);
        assert_eq!(requested.flute_count, 2);
        let tool = initial.tools.first().unwrap();
        let mut edited = CuttingToolDraft::from(tool);
        edited.feed_mm_per_min = 321.0;
        let state = store.update(&tool.id, edited).unwrap();
        assert_eq!(state.tools.first().unwrap().feed_mm_per_min, 321.0);
        assert!(state.tools.first().unwrap().factory_preset);
    }

    #[test]
    fn migrates_new_presets_once_and_respects_later_deletion() {
        let path = test_path("preset-catalog-migration");
        let mut legacy = StoredToolLibrary {
            preset_catalog_version: 0,
            ..StoredToolLibrary::default()
        };
        legacy
            .tools
            .retain(|tool| tool.id != CCT01_2F_06050_PRESET_ID);
        save_document(&path, &legacy).unwrap();

        let mut store = ToolLibraryStore::load(&path).unwrap();
        assert!(store.get(CCT01_2F_06050_PRESET_ID).is_some());
        store.delete(CCT01_2F_06050_PRESET_ID).unwrap();

        let reloaded = ToolLibraryStore::load(&path).unwrap();
        assert!(reloaded.get(CCT01_2F_06050_PRESET_ID).is_none());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn creates_updates_deletes_and_persists_custom_tools() {
        let path = test_path("crud");
        let mut store = ToolLibraryStore::load(&path).unwrap();
        let state = store.create(custom("Моя фреза")).unwrap();
        let id = state
            .tools
            .iter()
            .find(|tool| tool.name == "Моя фреза")
            .unwrap()
            .id
            .clone();
        let mut edited = custom("Моя фреза 2");
        edited.diameter_mm = 4.0;
        store.update(&id, edited).unwrap();
        let reloaded = ToolLibraryStore::load(&path).unwrap().state();
        assert_eq!(
            reloaded
                .tools
                .iter()
                .find(|tool| tool.id == id)
                .unwrap()
                .diameter_mm,
            4.0
        );
        let mut store = ToolLibraryStore::load(&path).unwrap();
        assert!(
            !store
                .delete(&id)
                .unwrap()
                .tools
                .iter()
                .any(|tool| tool.id == id)
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn restore_adds_deleted_presets_without_overwriting_edits() {
        let mut store = ToolLibraryStore::in_memory();
        let original = store.get("preset-carbide3d-102").unwrap().clone();
        let mut edited = CuttingToolDraft::from(&original);
        edited.feed_mm_per_min = 333.0;
        store.update(&original.id, edited).unwrap();
        store.delete("preset-carbide3d-mcfly").unwrap();
        let restored = store.restore_missing_presets().unwrap();
        assert_eq!(
            restored
                .tools
                .iter()
                .find(|tool| tool.id == original.id)
                .unwrap()
                .feed_mm_per_min,
            333.0
        );
        assert!(
            restored
                .tools
                .iter()
                .any(|tool| tool.id == "preset-carbide3d-mcfly")
        );
    }

    #[test]
    fn refreshes_only_legacy_unedited_preset_descriptions() {
        let path = test_path("preset-knowledge");
        let mut document = StoredToolLibrary::default();
        let legacy = document
            .tools
            .iter_mut()
            .find(|tool| tool.id == "preset-carbide3d-102")
            .unwrap();
        legacy.description = default_description(legacy.kind).to_owned();
        let customized = document
            .tools
            .iter_mut()
            .find(|tool| tool.id == "preset-carbide3d-201")
            .unwrap();
        customized.description = "Моё проверенное описание".to_owned();
        save_document(&path, &document).unwrap();

        let state = ToolLibraryStore::load(&path).unwrap().state();
        assert!(
            state
                .tools
                .iter()
                .find(|tool| tool.id == "preset-carbide3d-102")
                .unwrap()
                .description
                .starts_with("Компактная")
        );
        assert_eq!(
            state
                .tools
                .iter()
                .find(|tool| tool.id == "preset-carbide3d-201")
                .unwrap()
                .description,
            "Моё проверенное описание"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn rejects_duplicate_names_and_invalid_geometry() {
        let mut store = ToolLibraryStore::in_memory();
        let existing = store.state().tools[0].name.clone();
        assert!(matches!(
            store.create(custom(&existing)),
            Err(ToolLibraryError::DuplicateName(_))
        ));
        let mut invalid = custom("Invalid");
        invalid.stepover_percent = 100.0;
        assert!(matches!(
            store.create(invalid),
            Err(ToolLibraryError::InvalidValue {
                field: "stepoverPercent",
                ..
            })
        ));
    }
}
