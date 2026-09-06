use std::{collections::VecDeque, mem::size_of, sync::OnceLock};

use super::*;
use millo_gcode::{ProgramLine, ToolpathKind, ToolpathSegment};

pub const DOCUMENT_PAGE_SIZE: usize = 512;
const DISPLAY_POINT_BUDGET: usize = 50_000;
const DISPLAY_WARNING_BUDGET: usize = 2_000;
const CACHE_BYTES: usize = 768 * 1024 * 1024;
const CACHE_DOCUMENTS: usize = 4;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgramInput {
    pub source_name: String,
    #[serde(default)]
    pub source: String,
    pub program_id: Option<String>,
}

impl From<ProgramParseRequest> for ProgramInput {
    fn from(request: ProgramParseRequest) -> Self {
        Self {
            source_name: request.source_name,
            source: request.source,
            program_id: None,
        }
    }
}

pub struct ProgramDocument {
    pub id: String,
    pub source: Arc<str>,
    pub program: Arc<GcodeProgram>,
    bytes: usize,
}

#[derive(Default)]
struct ProgramDocuments {
    entries: VecDeque<Arc<ProgramDocument>>,
    sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub id: String,
    pub source_bytes: usize,
    pub page_size: usize,
    pub preview_sampled: bool,
    pub warning_count: usize,
    pub blocking_warning_count: usize,
    pub error_count: usize,
    pub managed_tool_change_count: usize,
    pub deepest_cutting_z: Option<f64>,
    pub tool_selections: Vec<ToolSelection>,
    pub tool_selection_coverage_line: usize,
    pub initial_tool_number: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSelection {
    pub source_line: usize,
    pub tool: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramDocumentView {
    #[serde(flatten)]
    program: GcodeProgram,
    document: DocumentMetadata,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramLinePage {
    pub program_id: String,
    pub start_index: usize,
    pub total_lines: usize,
    pub lines: Vec<ProgramLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramLineDetail {
    pub program_id: String,
    pub line: ProgramLine,
    pub toolpath: Vec<ToolpathSegment>,
}

fn documents() -> &'static StdMutex<ProgramDocuments> {
    static DOCUMENTS: OnceLock<StdMutex<ProgramDocuments>> = OnceLock::new();
    DOCUMENTS.get_or_init(Default::default)
}

impl ProgramDocuments {
    fn get(&mut self, id: &str) -> Result<Arc<ProgramDocument>, String> {
        let index = self.entries.iter().position(|entry| entry.id == id)
            .ok_or_else(|| "[PROGRAM_DOCUMENT_EXPIRED] Версия программы больше не загружена. Откройте файл повторно.".to_owned())?;
        let entry = self.entries.remove(index).expect("located document");
        self.entries.push_back(Arc::clone(&entry));
        Ok(entry)
    }

    fn insert(
        &mut self,
        source: Arc<str>,
        program: GcodeProgram,
        bytes: usize,
    ) -> Result<(Arc<ProgramDocument>, Vec<Arc<ProgramDocument>>), String> {
        if bytes > CACHE_BYTES {
            return Err(format!(
                "Программа требует больше {} MiB памяти документа. Разделите её на несколько файлов.",
                CACHE_BYTES / 1024 / 1024
            ));
        }
        self.sequence += 1;
        let mut evicted = Vec::new();
        while !self.entries.is_empty()
            && (self.entries.len() >= CACHE_DOCUMENTS
                || self
                    .entries
                    .iter()
                    .map(|entry| entry.bytes)
                    .sum::<usize>()
                    .saturating_add(bytes)
                    > CACHE_BYTES)
        {
            if let Some(entry) = self.entries.pop_front() {
                evicted.push(entry);
            }
        }
        let entry = Arc::new(ProgramDocument {
            id: format!("{}-{}", std::process::id(), self.sequence),
            source,
            program: Arc::new(program),
            bytes,
        });
        self.entries.push_back(Arc::clone(&entry));
        Ok((entry, evicted))
    }
}

fn resident_bytes(program: &GcodeProgram) -> usize {
    program.lines.capacity() * size_of::<ProgramLine>()
        + program
            .lines
            .iter()
            .map(|line| line.source.capacity() + line.normalized.capacity())
            .sum::<usize>()
        + program.execution_checkpoints.capacity()
            * size_of::<millo_gcode::ProgramExecutionCheckpoint>()
        + program.toolpath.capacity() * size_of::<ToolpathSegment>()
        + program
            .toolpath
            .iter()
            .map(|segment| segment.points.capacity() * size_of::<millo_gcode::ProgramPoint>())
            .sum::<usize>()
        + program
            .warnings
            .iter()
            .map(|warning| size_of::<millo_gcode::ProgramWarning>() + warning.message.capacity())
            .sum::<usize>()
}

pub(super) async fn resolve_program(
    request: ProgramInput,
    options: ProgramParseOptions,
) -> Result<Arc<ProgramDocument>, String> {
    if let Some(entry) = resolve_existing(&request)? {
        if entry.program.block_delete_enabled == options.block_delete {
            return Ok(entry);
        }
        return parse_document(
            entry.program.source_name.clone(),
            Arc::clone(&entry.source),
            options,
        )
        .await;
    }
    parse_document(request.source_name, Arc::from(request.source), options).await
}

async fn parse_document(
    source_name: String,
    source: Arc<str>,
    options: ProgramParseOptions,
) -> Result<Arc<ProgramDocument>, String> {
    background_compute::run("Program document parser", move || {
        parse_and_store(source_name, source, options)
    })
    .await
}

fn parse_and_store(
    source_name: String,
    source: Arc<str>,
    options: ProgramParseOptions,
) -> Result<Arc<ProgramDocument>, String> {
    let program = parse_program_with_options(
        ProgramParseRequest {
            source_name,
            source: source.to_string(),
        },
        options,
    )
    .map_err(|error| error.to_string())?;
    let bytes = resident_bytes(&program).saturating_add(source.len());
    if bytes > CACHE_BYTES {
        return Err(format!(
            "Программа требует больше {} MiB памяти документа. Разделите её на несколько файлов.",
            CACHE_BYTES / 1024 / 1024
        ));
    }
    // Estimation and deallocation of large revisions must not hold the cache lock.
    let (entry, evicted) = documents()
        .lock()
        .map_err(|e| e.to_string())?
        .insert(source, program, bytes)?;
    drop(evicted);
    Ok(entry)
}

fn resolve_existing(request: &ProgramInput) -> Result<Option<Arc<ProgramDocument>>, String> {
    if let Some(id) = &request.program_id {
        if !request.source.is_empty() {
            return Err(
                "Передайте либо идентификатор программы, либо исходный код, но не оба.".to_owned(),
            );
        }
        let entry = documents().lock().map_err(|e| e.to_string())?.get(id)?;
        if entry.program.source_name != request.source_name {
            return Err("Идентификатор не соответствует имени программы.".to_owned());
        }
        Ok(Some(entry))
    } else {
        Ok(None)
    }
}

pub(super) fn resolve_program_blocking(
    request: ProgramInput,
    options: ProgramParseOptions,
) -> Result<Arc<ProgramDocument>, String> {
    if let Some(entry) = resolve_existing(&request)? {
        if entry.program.block_delete_enabled == options.block_delete {
            return Ok(entry);
        }
        return parse_and_store(
            entry.program.source_name.clone(),
            Arc::clone(&entry.source),
            options,
        );
    }
    parse_and_store(request.source_name, Arc::from(request.source), options)
}

#[tauri::command]
pub async fn open_gcode_document(
    request: ProgramInput,
    options: Option<ProgramParseOptions>,
) -> Result<ProgramDocumentView, String> {
    let document = resolve_program(request, options.unwrap_or_default()).await?;
    background_compute::run("Program display preparation", move || {
        Ok::<_, String>(document_view(&document))
    })
    .await
}

#[tauri::command]
pub fn program_line_page(
    program_id: String,
    start_index: usize,
    count: usize,
) -> Result<ProgramLinePage, String> {
    if count == 0 || count > DOCUMENT_PAGE_SIZE {
        return Err(format!("Запросите от 1 до {DOCUMENT_PAGE_SIZE} строк."));
    }
    let document = documents()
        .lock()
        .map_err(|e| e.to_string())?
        .get(&program_id)?;
    let total_lines = document.program.lines.len();
    if start_index > total_lines {
        return Err("Строка находится за пределами программы.".to_owned());
    }
    let end = start_index.saturating_add(count).min(total_lines);
    Ok(ProgramLinePage {
        program_id,
        start_index,
        total_lines,
        lines: document.program.lines[start_index..end].to_vec(),
    })
}

#[tauri::command]
pub fn program_line_detail(
    program_id: String,
    source_line: usize,
) -> Result<ProgramLineDetail, String> {
    let document = documents()
        .lock()
        .map_err(|e| e.to_string())?
        .get(&program_id)?;
    let line = source_line
        .checked_sub(1)
        .and_then(|index| document.program.lines.get(index))
        .ok_or_else(|| "Строка находится за пределами программы.".to_owned())?
        .clone();
    let paths = &document.program.toolpath;
    let start = paths.partition_point(|segment| segment.source_line < source_line);
    let end = paths.partition_point(|segment| segment.source_line <= source_line);
    let (toolpath, _) = display_toolpath(&paths[start..end], DISPLAY_POINT_BUDGET);
    Ok(ProgramLineDetail {
        program_id,
        line,
        toolpath,
    })
}

#[tauri::command]
pub async fn save_processed_gcode_document(
    request: ProgramInput,
    source_name: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<GeneratedGcodeSaveOutcome>, String> {
    // Resolve the exact parsed revision, including its Block Delete selection.
    let id = request
        .program_id
        .as_ref()
        .ok_or_else(|| "Processed export requires a parsed document".to_owned())?;
    let existing = documents()
        .lock()
        .map_err(|error| error.to_string())?
        .get(id)?;
    let document = resolve_program(
        request,
        ProgramParseOptions {
            block_delete: existing.program.block_delete_enabled,
        },
    )
    .await?;
    let source = background_compute::run("Processed G-code export", move || {
        Ok::<_, String>(processed_source(&document.program))
    })
    .await?;
    program_io::save_validated_gcode(
        ProgramParseRequest {
            source_name,
            source,
        },
        &app,
        &state.audit,
        "storage.processed_gcode_saved",
        "Full processed G-code saved",
    )
    .await
}

fn processed_source(program: &GcodeProgram) -> String {
    let mut source = String::new();
    for line in &program.lines {
        if !line.block_deleted && !line.normalized.is_empty() {
            source.push_str(&line.normalized);
            source.push('\n');
        }
    }
    source
}

fn document_view(document: &ProgramDocument) -> ProgramDocumentView {
    let program = &document.program;
    let (toolpath, preview_sampled) = display_toolpath(&program.toolpath, DISPLAY_POINT_BUDGET);
    let mut previous_tool = None;
    let mut tool_selections = Vec::new();
    let mut tool_selection_coverage_line = program.summary.line_count;
    for line in program
        .lines
        .iter()
        .filter(|line| line.executable && !line.block_deleted)
    {
        if !line.normalized.contains('T') {
            continue;
        }
        let selected = line
            .normalized
            .split_whitespace()
            .filter_map(tool_word)
            .next_back();
        if selected.is_some() && selected != previous_tool {
            if tool_selections.len() >= 16_384 {
                tool_selection_coverage_line = line.source_line - 1;
                break;
            }
            previous_tool = selected;
            tool_selections.push(ToolSelection {
                source_line: line.source_line,
                tool: selected,
            });
        }
    }
    ProgramDocumentView {
        program: GcodeProgram {
            source_name: program.source_name.clone(),
            block_delete_enabled: program.block_delete_enabled,
            lines: program
                .lines
                .iter()
                .take(DOCUMENT_PAGE_SIZE)
                .cloned()
                .collect(),
            warnings: program
                .warnings
                .iter()
                .take(DISPLAY_WARNING_BUDGET)
                .cloned()
                .collect(),
            features: program.features.clone(),
            summary: program.summary.clone(),
            toolpath,
            execution_checkpoints: Vec::new(),
        },
        document: DocumentMetadata {
            id: document.id.clone(),
            source_bytes: document.source.len(),
            page_size: DOCUMENT_PAGE_SIZE,
            preview_sampled,
            warning_count: program.warnings.len(),
            blocking_warning_count: program
                .warnings
                .iter()
                .filter(|warning| warning.severity != millo_gcode::ProgramWarningSeverity::Warning)
                .count(),
            error_count: program
                .warnings
                .iter()
                .filter(|warning| warning.severity == millo_gcode::ProgramWarningSeverity::Error)
                .count(),
            managed_tool_change_count: program
                .warnings
                .iter()
                .filter(|warning| warning.code == millo_gcode::ProgramWarningCode::ToolChange)
                .count(),
            deepest_cutting_z: program
                .toolpath
                .iter()
                .filter(|segment| segment.kind != ToolpathKind::Rapid)
                .flat_map(|segment| &segment.points)
                .map(|point| point.z)
                .filter(|z| *z < -1e-9)
                .reduce(f64::min),
            tool_selections,
            tool_selection_coverage_line,
            initial_tool_number: initial_tool_number(program),
        },
    }
}

fn initial_tool_number(program: &GcodeProgram) -> Option<u8> {
    let mut selected = None;
    for line in program
        .lines
        .iter()
        .filter(|line| line.executable && !line.block_deleted)
    {
        let mut boundary = false;
        for word in line.normalized.split_whitespace() {
            if let Some(value) = tool_word(word) {
                selected = Some(value);
            }
            boundary |= word.strip_prefix('M').and_then(|v| v.parse::<f64>().ok()) == Some(6.0)
                || matches!(
                    word.strip_prefix('G').and_then(|v| v.parse::<f64>().ok()),
                    Some(1.0 | 2.0 | 3.0)
                );
        }
        if boundary {
            break;
        }
    }
    selected
}

fn tool_word(word: &str) -> Option<u8> {
    word.strip_prefix('T')?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && value.fract() == 0.0 && (0.0..=255.0).contains(value))
        .map(|value| value as u8)
}

fn display_toolpath(toolpath: &[ToolpathSegment], budget: usize) -> (Vec<ToolpathSegment>, bool) {
    let total = toolpath
        .iter()
        .map(|segment| segment.points.len())
        .sum::<usize>();
    if total <= budget {
        return (toolpath.to_vec(), false);
    }
    // Independent segments retain their source association; never join omitted moves.
    let stride = toolpath.len().div_ceil((budget / 8).max(1));
    let candidates = toolpath.iter().step_by(stride.max(1));
    let count = candidates.clone().count().max(1);
    let per_segment = (budget / count).max(2);
    let mut result = Vec::with_capacity(count);
    for segment in candidates {
        let points = if segment.points.len() > per_segment {
            let last = segment.points.len() - 1;
            (0..per_segment)
                .map(|i| segment.points[i * last / (per_segment - 1)])
                .collect()
        } else {
            segment.points.clone()
        };
        result.push(ToolpathSegment {
            source_line: segment.source_line,
            optional_block: segment.optional_block,
            kind: segment.kind,
            points,
            rotary: segment.rotary,
            distance_mm: segment.distance_mm,
            feed_rate_mm_per_min: segment.feed_rate_mm_per_min,
            estimated_duration_seconds: segment.estimated_duration_seconds,
        });
    }
    (result, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "test.nc".into(),
            source: source.into(),
        })
        .unwrap()
    }

    #[test]
    fn display_is_bounded_without_truncating_execution() {
        let source = format!("G21 G90 G17 G94\n{}", "G1 X1 F100\nG1 X0\n".repeat(10_000));
        let mut store = ProgramDocuments::default();
        let program = parsed(&source);
        let bytes = resident_bytes(&program) + source.len();
        let (entry, _) = store
            .insert(Arc::from(source.as_str()), program, bytes)
            .unwrap();
        let view = document_view(&entry);
        assert_eq!(view.program.lines.len(), DOCUMENT_PAGE_SIZE);
        assert_eq!(view.program.summary.line_count, 20_001);
        assert_eq!(entry.program.lines.len(), 20_001);
        let (sample, sampled) = display_toolpath(&entry.program.toolpath, 1024);
        assert!(sampled);
        assert!(
            sample
                .iter()
                .map(|segment| segment.points.len())
                .sum::<usize>()
                <= 1024
        );
        assert_eq!(sample[0].source_line, 2);
        assert!(Arc::ptr_eq(&entry, &store.get(&entry.id).unwrap()));
    }

    #[test]
    fn eviction_does_not_modify_a_document_owned_by_execution() {
        let mut store = ProgramDocuments::default();
        let (first, _) = store
            .insert(Arc::from("G1 X1"), parsed("G1 X1"), 1024)
            .unwrap();
        for _ in 0..CACHE_DOCUMENTS {
            store
                .insert(Arc::from("G1 X2"), parsed("G1 X2"), 1024)
                .unwrap();
        }
        assert!(store.get(&first.id).is_err());
        assert_eq!(first.source.as_ref(), "G1 X1");
    }

    #[test]
    fn oversized_revision_does_not_evict_current_document() {
        let mut store = ProgramDocuments::default();
        let (first, _) = store
            .insert(Arc::from("G1 X1"), parsed("G1 X1"), 1024)
            .unwrap();
        assert!(
            store
                .insert(Arc::from("G1 X2"), parsed("G1 X2"), CACHE_BYTES + 1)
                .is_err()
        );
        assert!(Arc::ptr_eq(&first, &store.get(&first.id).unwrap()));
    }

    #[test]
    fn exact_pages_and_line_detail_use_full_document() {
        let source = format!(
            "G21 G90 G17 G94\n{}G1 X99 A180 F100",
            "G1 X1 F100\nG1 X0\n".repeat(1000)
        );
        let document = parse_and_store(
            "test.nc".into(),
            Arc::from(source),
            ProgramParseOptions::default(),
        )
        .unwrap();
        let last = document.program.lines.len();
        let page = program_line_page(document.id.clone(), last - 1, 512).unwrap();
        assert_eq!(page.total_lines, last);
        assert_eq!(page.lines.len(), 1);
        assert_eq!(page.lines[0].source_line, last);
        let detail = program_line_detail(document.id.clone(), last).unwrap();
        assert_eq!(detail.toolpath[0].rotary.unwrap().end_degrees, 180.0);
        assert!(program_line_page(document.id.clone(), 0, 513).is_err());
        assert!(program_line_detail(document.id.clone(), last + 1).is_err());
        let request = ProgramInput {
            source_name: "test.nc".into(),
            source: String::new(),
            program_id: Some(document.id.clone()),
        };
        assert!(Arc::ptr_eq(
            &document,
            &resolve_program_blocking(request.clone(), ProgramParseOptions::default()).unwrap()
        ));
        assert!(
            resolve_program_blocking(
                ProgramInput {
                    source_name: "wrong.nc".into(),
                    ..request.clone()
                },
                ProgramParseOptions::default()
            )
            .is_err()
        );
        assert!(
            resolve_program_blocking(
                ProgramInput {
                    source: "G1 X999".into(),
                    ..request
                },
                ProgramParseOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn processed_export_keeps_all_pages_and_selected_block_delete_policy() {
        let source = format!(
            "G21 G90 G94\n{}G1 X17 A90 F100\n/G1 X999 F100\nM2",
            "G1 X0 F100\n".repeat(2000)
        );
        let program = parse_program_with_options(
            ProgramParseRequest {
                source_name: "export.nc".into(),
                source,
            },
            ProgramParseOptions { block_delete: true },
        )
        .unwrap();
        let processed = processed_source(&program);
        assert_eq!(processed.lines().count(), 2003);
        assert!(processed.contains("G1 X17 A90 F100"));
        assert!(!processed.contains("999"));
        assert!(processed.ends_with("M2\n"));
    }

    #[test]
    fn initial_tool_after_a_long_comment_header_is_not_lost_to_paging() {
        let source = format!("{}T7.0 M6\nG1 X1 F100\nT2 M6", "(header)\n".repeat(600));
        let program = parsed(&source);
        assert_eq!(initial_tool_number(&program), Some(7));
        assert_eq!(initial_tool_number(&parsed("G1 X1 F100\nT2 M6")), None);
    }

    #[test]
    #[ignore = "one-million-line display/memory benchmark; run explicitly in release"]
    fn million_line_document_benchmark() {
        let start = Instant::now();
        let source = format!(
            "G21 G90 G17 G94\n{}G1 X17 A90 F100\nM2",
            "G1 X0 F100\nG1 X1 F100\n".repeat(500_000)
        );
        let document = parse_and_store(
            "million.nc".into(),
            Arc::from(source),
            ProgramParseOptions::default(),
        )
        .unwrap();
        let parse_time = start.elapsed();
        let view = document_view(&document);
        let json = serde_json::to_vec(&view).unwrap();
        assert_eq!(view.program.lines.len(), DOCUMENT_PAGE_SIZE);
        assert_eq!(view.program.summary.line_count, 1_000_003);
        assert!(
            view.program
                .toolpath
                .iter()
                .map(|segment| segment.points.len())
                .sum::<usize>()
                <= DISPLAY_POINT_BUDGET
        );
        assert!(view.document.preview_sampled);
        assert!(json.len() < 8 * 1024 * 1024);
        assert_eq!(
            program_line_page(document.id.clone(), 1_000_001, 512)
                .unwrap()
                .lines
                .len(),
            2
        );
        assert_eq!(
            program_line_detail(document.id.clone(), 1_000_002)
                .unwrap()
                .toolpath[0]
                .rotary
                .unwrap()
                .end_degrees,
            90.0
        );
        eprintln!(
            "million document: resident_estimate={}B source={}B display={}B parse={:?} total={:?}",
            document.bytes,
            document.source.len(),
            json.len(),
            parse_time,
            start.elapsed()
        );
    }
}
