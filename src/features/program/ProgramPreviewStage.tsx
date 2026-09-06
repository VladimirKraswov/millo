import { Play, X } from "lucide-react";
import { lazy, Suspense, useMemo } from "react";
import { FeatureErrorBoundary } from "../../components/FeatureErrorBoundary";

import type { Position } from "../../shared/machine";
import type { GcodeProgram, ProgramLine, ToolpathSegment } from "../../shared/program";
import type { PreviewView } from "./ToolpathPreview";
import type { ProgramToolVisualization } from "./programToolVisualizationModel";
import { buildRotarySelectionReadModel, formatRotaryDegrees } from "./toolpathReadModel";

const ToolpathPreview = lazy(async () => {
  const module = await import("./ToolpathPreview");
  return { default: module.ToolpathPreview };
});

interface ProgramPreviewStageProps {
  readonly cuttingDepthAdjustmentMm: number;
  readonly onClearSelection: () => void;
  readonly onSafeStart: () => void;
  readonly onSelectSourceLine: (sourceLine: number) => void;
  readonly program: GcodeProgram;
  readonly safeStartAvailable: boolean;
  readonly selectedMotionCount: number;
  readonly selectedProgramLine?: ProgramLine;
  readonly selectedSourceLine?: number;
  readonly selectedToolpath?: readonly ToolpathSegment[];
  readonly toolCoordinateSystem?: string;
  readonly toolPosition?: Position;
  readonly toolVisualization: ProgramToolVisualization;
  readonly view: PreviewView;
}

export function ProgramPreviewStage({
  cuttingDepthAdjustmentMm,
  onClearSelection,
  onSafeStart,
  onSelectSourceLine,
  program,
  safeStartAvailable,
  selectedMotionCount,
  selectedProgramLine,
  selectedSourceLine,
  selectedToolpath,
  toolCoordinateSystem,
  toolPosition,
  toolVisualization,
  view,
}: ProgramPreviewStageProps) {
  const bounds = program.summary.bounds;
  const pathDistance = program.summary.rapidDistanceMm + program.summary.cuttingDistanceMm;
  const rotaryBounds = program.summary.rotaryBounds;
  const hasRotary = Boolean(program.features.usesRotaryA || rotaryBounds);
  const previewSampled = program.document?.previewSampled;
  const selectedRotary = useMemo(
    () => buildRotarySelectionReadModel(program, selectedSourceLine, selectedToolpath),
    [program, selectedSourceLine, selectedToolpath],
  );

  return (
    <div className={`program-preview-stage${hasRotary ? " has-rotary" : ""}${previewSampled ? " has-sampled-preview" : ""}`}>
      <FeatureErrorBoundary name="Траекторию">
      <Suspense
        fallback={<div className="toolpath-preview is-loading">Загрузка траектории...</div>}
      >
        <ToolpathPreview
          cuttingDepthAdjustmentMm={cuttingDepthAdjustmentMm}
          onSelectSourceLine={onSelectSourceLine}
          program={program}
          selectedSourceLine={selectedSourceLine}
          selectedToolpath={selectedToolpath}
          toolCoordinateSystem={toolCoordinateSystem}
          toolPosition={toolPosition}
          toolVisualization={toolVisualization}
          view={view}
        />
      </Suspense>
      </FeatureErrorBoundary>
      <div className="preview-legend" aria-label="Обозначения траектории">
        <span className="is-cut">Рабочий ход</span>
        <span className="is-rapid">Быстрый ход</span>
        {previewSampled && (
          <strong className="preview-sampling" title="Показана выборка траектории. Границы и сводка рассчитаны по всей программе; выполнение использует все строки.">
            Обзорная траектория
          </strong>
        )}
      </div>
      {selectedProgramLine && (
        <div className="preview-selection" role="status">
          <span>L{selectedProgramLine.sourceLine}</span>
          <code title={selectedProgramLine.source}>
            {selectedProgramLine.source || "Пустая строка"}
          </code>
          <small>
            {selectedMotionCount > 0
              ? formatSegmentCount(selectedMotionCount)
              : "В этой строке нет движения"}
          </small>
          {safeStartAvailable && (
            <button
              className="preview-safe-start"
              onClick={onSafeStart}
              title="Сформировать безопасный запуск с этого участка"
              type="button"
            >
              <Play aria-hidden="true" size={12} />
              С этого участка
            </button>
          )}
          <button
            aria-label="Очистить выбор строки"
            onClick={onClearSelection}
            title="Очистить выбор строки"
            type="button"
          >
            <X aria-hidden="true" size={12} />
          </button>
          {selectedRotary.length > 0 && (
            <dl className="preview-rotary-selection" aria-label="Поворот A выбранной строки">
              {selectedRotary.map((rotary, index) => (
                <div key={index}>
                  <dt>A{selectedRotary.length > 1 ? ` · ${index + 1}` : ""}</dt>
                  <dd>
                    <span>Начало {formatRotaryDegrees(rotary.startDegrees)}</span>
                    <span>Конец {formatRotaryDegrees(rotary.endDegrees)}</span>
                  </dd>
                </div>
              ))}
            </dl>
          )}
        </div>
      )}
      {hasRotary && (
        <dl className="program-rotary-metrics" aria-label="Поворотная ось программы">
          <div>
            <dt>Предпросмотр</dt>
            <dd>Проекция XYZ</dd>
          </div>
          <div>
            <dt>Диапазон A</dt>
            <dd>{formatRotaryDegrees(rotaryBounds?.minDegrees)} … {formatRotaryDegrees(rotaryBounds?.maxDegrees)}</dd>
          </div>
          <div>
            <dt>Путь A</dt>
            <dd>{formatRotaryDegrees(program.summary.rotaryTravelDegrees)}</dd>
          </div>
        </dl>
      )}
      <dl className="program-metrics">
        <div>
          <dt>Строки</dt>
          <dd>{program.summary.lineCount}</dd>
        </div>
        <div>
          <dt>Время</dt>
          <dd>
            {formatDuration(
              program.summary.estimatedTotalTimeSeconds,
              program.summary.timeEstimateComplete,
            )}
          </dd>
        </div>
        <div>
          <dt>Траектория</dt>
          <dd>{formatDistance(pathDistance)}</dd>
        </div>
        <div>
          <dt>Размер XYZ</dt>
          <dd>
            {bounds
              ? `${bounds.size.x.toFixed(1)} × ${bounds.size.y.toFixed(1)} × ${bounds.size.z.toFixed(1)}`
              : "--"}
          </dd>
        </div>
      </dl>
    </div>
  );
}

const formatDistance = (value: number): string =>
  value >= 1_000 ? `${(value / 1_000).toFixed(2)} m` : `${value.toFixed(1)} mm`;

const formatSegmentCount = (count: number): string => {
  const lastTwo = count % 100;
  const last = count % 10;
  const noun = lastTwo >= 11 && lastTwo <= 14
    ? "сегментов"
    : last === 1
      ? "сегмент"
      : last >= 2 && last <= 4
        ? "сегмента"
        : "сегментов";
  return `${count} ${noun} траектории`;
};

const formatDuration = (seconds: number, complete: boolean): string => {
  const rounded = Math.max(0, Math.round(seconds));
  const hours = Math.floor(rounded / 3_600);
  const minutes = Math.floor((rounded % 3_600) / 60);
  const remainder = rounded % 60;
  const value = hours > 0
    ? `${hours} ч ${minutes} мин`
    : minutes > 0
      ? `${minutes} мин ${remainder} с`
      : `${remainder} с`;
  return `${complete ? "~" : ">="}${value}`;
};
