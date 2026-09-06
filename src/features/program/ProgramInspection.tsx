import { ChevronDown, ShieldAlert, TriangleAlert, Wrench } from "lucide-react";

import type { GcodeProgram, ProgramWarning } from "../../shared/program";
import type { RunPreflightReport } from "../../shared/realRun";
import { PagedProgramLineTable } from "./PagedProgramLineTable";
import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import { ProgramPreflightReport } from "./ProgramPreflightReport";
import {
  formatProgramDiagnostics,
  programDiagnosticsSummary,
  programWarningPresentation,
} from "./programDiagnosticsModel";

export type ProgramDiagnosticView = "lines" | "warnings" | "preflight";

interface ProgramInspectionProps {
  readonly gateway?: ProgramGateway;
  readonly diagnosticView: ProgramDiagnosticView;
  readonly motionSourceLines: ReadonlySet<number>;
  readonly onOpenChange: (open: boolean) => void;
  readonly onSelectSourceLine: (sourceLine?: number) => void;
  readonly onView: (view: ProgramDiagnosticView) => void;
  readonly open: boolean;
  readonly program: GcodeProgram;
  readonly source?: string;
  readonly realRunTarget: boolean;
  readonly report?: RunPreflightReport;
  readonly selectedSourceLine?: number;
}

export function ProgramInspection({
  gateway,
  diagnosticView,
  motionSourceLines,
  onOpenChange,
  onSelectSourceLine,
  onView,
  open,
  program,
  source,
  realRunTarget,
  report,
  selectedSourceLine,
}: ProgramInspectionProps) {
  const diagnostics = programDiagnosticsSummary(program);
  const diagnosticsLabel = formatProgramDiagnostics(diagnostics);
  return (
    <details
      className="program-inspection"
      onToggle={(event) => onOpenChange(event.currentTarget.open)}
      open={open}
    >
      <summary>
        <span>Программа и диагностика</span>
        <code>
          {program.summary.lineCount} строк
          {diagnosticsLabel ? ` · ${diagnosticsLabel}` : ""}
        </code>
        <ChevronDown aria-hidden="true" size={13} />
      </summary>
      <div
        aria-label="Раздел диагностики программы"
        className={`program-diagnostic-tabs${realRunTarget ? " has-preflight" : ""}`}
        role="tablist"
      >
        <DiagnosticTab
          active={diagnosticView === "lines"}
          controls="program-lines-panel"
          count={program.summary.lineCount}
          id="program-lines-tab"
          label="Строки"
          onClick={() => onView("lines")}
        />
        <DiagnosticTab
          active={diagnosticView === "warnings"}
          controls="program-warnings-panel"
          count={program.document?.warningCount ?? program.warnings.length}
          id="program-warnings-tab"
          label="Диагностика"
          onClick={() => onView("warnings")}
        />
        {realRunTarget && (
          <DiagnosticTab
            active={diagnosticView === "preflight"}
            controls="program-preflight-panel"
            count={report?.blockerCount ?? "--"}
            disabled={!report}
            id="program-preflight-tab"
            label="Проверка"
            onClick={() => onView("preflight")}
          />
        )}
      </div>
      <div
        aria-labelledby="program-lines-tab"
        className="program-lines-panel"
        hidden={diagnosticView !== "lines"}
        id="program-lines-panel"
        role="tabpanel"
      >
        <PagedProgramLineTable
          program={program}
          source={source}
          gateway={gateway}
          motionSourceLines={motionSourceLines}
          onSelect={(sourceLine) =>
            onSelectSourceLine(selectedSourceLine === sourceLine ? undefined : sourceLine)
          }
          selectedSourceLine={selectedSourceLine}
        />
      </div>
      <div
        aria-labelledby="program-warnings-tab"
        className="program-warnings"
        hidden={diagnosticView !== "warnings"}
        id="program-warnings-panel"
        role="tabpanel"
      >
        {program.document && program.document.warningCount > program.warnings.length && <div className="program-page-error">Показаны первые {program.warnings.length} замечаний из {program.document.warningCount}. Проверка перед запуском учитывает все строки.</div>}
        {program.warnings.length === 0 ? (
          <div className="warnings-empty">Парсер не нашёл замечаний</div>
        ) : (
          program.warnings.map((warning, index) => (
            <WarningRow
              key={`${warning.sourceLine}-${warning.code}-${index}`}
              onSelect={() => onSelectSourceLine(warning.sourceLine)}
              selected={selectedSourceLine === warning.sourceLine}
              warning={warning}
            />
          ))
        )}
      </div>
      {realRunTarget && (
        <div
          aria-labelledby="program-preflight-tab"
          hidden={diagnosticView !== "preflight"}
          id="program-preflight-panel"
          role="tabpanel"
        >
          {report && (
            <ProgramPreflightReport
              onSelectSourceLine={(sourceLine) => {
                onSelectSourceLine(sourceLine);
                onView("lines");
              }}
              report={report}
            />
          )}
        </div>
      )}
    </details>
  );
}

function DiagnosticTab({
  active,
  controls,
  count,
  disabled = false,
  id,
  label,
  onClick,
}: {
  readonly active: boolean;
  readonly controls: string;
  readonly count: number | string;
  readonly disabled?: boolean;
  readonly id: string;
  readonly label: string;
  readonly onClick: () => void;
}) {
  return (
    <button
      aria-controls={controls}
      aria-selected={active}
      disabled={disabled}
      id={id}
      onClick={onClick}
      role="tab"
      type="button"
    >
      {label} <strong>{count}</strong>
    </button>
  );
}

function WarningRow({
  onSelect,
  selected,
  warning,
}: {
  readonly onSelect: () => void;
  readonly selected: boolean;
  readonly warning: ProgramWarning;
}) {
  const presentation = programWarningPresentation(warning);
  return (
    <button
      aria-pressed={selected}
      className={`program-warning is-${presentation.kind}`}
      onClick={onSelect}
      type="button"
    >
      <span className="warning-line">L{warning.sourceLine}</span>
      {presentation.kind === "managed" ? (
        <Wrench aria-hidden="true" size={13} />
      ) : warning.severity === "safety" ? (
        <ShieldAlert aria-hidden="true" size={13} />
      ) : (
        <TriangleAlert aria-hidden="true" size={13} />
      )}
      <div>
        <strong>{presentation.title}</strong>
        <span>{presentation.detail}</span>
      </div>
    </button>
  );
}
