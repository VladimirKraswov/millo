import { TriangleAlert } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type UIEvent } from "react";

import type { ProgramLine } from "../../shared/program";
import {
  PROGRAM_LINE_ROW_HEIGHT,
  buildProgramLineWindow,
  findProgramLineIndex,
} from "./programLineTableModel";

interface ProgramLineTableProps {
  readonly lines: readonly ProgramLine[];
  readonly motionSourceLines: ReadonlySet<number>;
  readonly onSelect: (sourceLine: number) => void;
  readonly selectedSourceLine?: number;
}

export function ProgramLineTable({
  lines,
  motionSourceLines,
  onSelect,
  selectedSourceLine,
}: ProgramLineTableProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(
    PROGRAM_LINE_ROW_HEIGHT * 6,
  );
  const selectedIndex = useMemo(
    () => findProgramLineIndex(lines, selectedSourceLine),
    [lines, selectedSourceLine],
  );
  const window = useMemo(
    () => buildProgramLineWindow(lines, scrollTop, viewportHeight),
    [lines, scrollTop, viewportHeight],
  );

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const observer = new ResizeObserver(([entry]) => {
      setViewportHeight(entry.contentRect.height);
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || selectedIndex === undefined) return;
    const rowTop = selectedIndex * PROGRAM_LINE_ROW_HEIGHT;
    const rowBottom = rowTop + PROGRAM_LINE_ROW_HEIGHT;
    if (rowTop < viewport.scrollTop) {
      viewport.scrollTop = rowTop;
      setScrollTop(rowTop);
    } else if (rowBottom > viewport.scrollTop + viewport.clientHeight) {
      const next = rowBottom - viewport.clientHeight;
      viewport.scrollTop = next;
      setScrollTop(next);
    }
  }, [selectedIndex]);

  const onScroll = (event: UIEvent<HTMLDivElement>) => {
    setScrollTop(event.currentTarget.scrollTop);
  };

  return (
    <div
      aria-label="Program lines"
      className="program-line-table"
      onScroll={onScroll}
      ref={viewportRef}
      role="listbox"
    >
      <div
        className="program-line-spacer"
        style={{ height: `${window.totalHeightPx}px` }}
      >
        <div
          className="program-line-window"
          style={{ transform: `translateY(${window.offsetPx}px)` }}
        >
          {window.lines.map((line, index) => {
            const absoluteIndex = window.startIndex + index;
            const selected = line.sourceLine === selectedSourceLine;
            const hasMotion = motionSourceLines.has(line.sourceLine);
            return (
              <button
                aria-label={`Line ${line.sourceLine}: ${line.source || "empty line"}`}
                aria-posinset={absoluteIndex + 1}
                aria-selected={selected}
                aria-setsize={lines.length}
                className={`program-line-row${selected ? " is-selected" : ""}${line.executable ? "" : " is-comment"}`}
                key={line.sourceLine}
                onClick={() => onSelect(line.sourceLine)}
                role="option"
                style={{ height: `${PROGRAM_LINE_ROW_HEIGHT}px` }}
                type="button"
              >
                <span className="program-line-number">{line.sourceLine}</span>
                <i
                  aria-label={hasMotion ? "Preview motion" : undefined}
                  className={hasMotion ? "has-motion" : undefined}
                />
                <code title={line.source}>{line.source || " "}</code>
                {line.warningCount > 0 && (
                  <span className="program-line-warning" title="Parser warning">
                    <TriangleAlert aria-hidden="true" size={10} />
                    {line.warningCount}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
