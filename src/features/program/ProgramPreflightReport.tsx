import { ChevronDown, CircleAlert, CircleCheck } from "lucide-react";

import type { RunPreflightReport } from "../../shared/realRun";
import {
  presentPreflightReport,
  type PresentedPreflightCheck,
} from "./preflightPresentationModel";

interface ProgramPreflightReportProps {
  readonly onSelectSourceLine: (sourceLine: number) => void;
  readonly report: RunPreflightReport;
}

function CheckRow({
  check,
  onSelectSourceLine,
}: {
  readonly check: PresentedPreflightCheck;
  readonly onSelectSourceLine: (sourceLine: number) => void;
}) {
  const content = (
    <>
      {check.level === "pass" ? (
        <CircleCheck aria-hidden="true" size={13} />
      ) : (
        <CircleAlert aria-hidden="true" size={13} />
      )}
      <span>
        <strong>{check.title}</strong>
        <small>{check.detail}</small>
      </span>
      {check.sourceLine !== undefined && <code>L{check.sourceLine}</code>}
    </>
  );

  return check.sourceLine !== undefined ? (
    <button
      className={`real-run-check is-${check.level}`}
      onClick={() => onSelectSourceLine(check.sourceLine as number)}
      type="button"
    >
      {content}
    </button>
  ) : (
    <div className={`real-run-check is-${check.level}`}>{content}</div>
  );
}

function Checks({
  checks,
  onSelectSourceLine,
}: {
  readonly checks: readonly PresentedPreflightCheck[];
  readonly onSelectSourceLine: (sourceLine: number) => void;
}) {
  return checks.map((check) => (
    <CheckRow check={check} key={check.id} onSelectSourceLine={onSelectSourceLine} />
  ));
}

export function ProgramPreflightReport({
  onSelectSourceLine,
  report,
}: ProgramPreflightReportProps) {
  const presented = presentPreflightReport(report);
  return (
    <div className="real-run-checks">
      <div className={`preflight-summary${report.ready ? " is-ready" : " is-blocked"}`}>
        {report.ready ? (
          <CircleCheck aria-hidden="true" size={15} />
        ) : (
          <CircleAlert aria-hidden="true" size={15} />
        )}
        <span>
          <strong>{presented.title}</strong>
          <small>{presented.summary}</small>
        </span>
      </div>
      <Checks checks={presented.attention} onSelectSourceLine={onSelectSourceLine} />
      {presented.passed.length > 0 && (
        <details className="preflight-passed">
          <summary>
            <CircleCheck aria-hidden="true" size={13} />
            <span>Успешные проверки</span>
            <code>{presented.passed.length}</code>
            <ChevronDown aria-hidden="true" size={13} />
          </summary>
          <div>
            <Checks checks={presented.passed} onSelectSourceLine={onSelectSourceLine} />
          </div>
        </details>
      )}
    </div>
  );
}
