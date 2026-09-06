import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  normalizeTextSelection, replaceTextSelection, sourceLineAtOffset, sourceOffsetAtLine,
  type TextEdit, type TextSelection,
} from "./programEditorModel";
import {
  buildProgramEditorLineIndex, editProgramEditorPage, pageOffsetToSource,
  programEditorPageAtLine, sourceOffsetToPage, textChange,
} from "./programEditorPaging";

const caretOffset = (selection: TextSelection) =>
  selection.direction === "backward" ? selection.start : selection.end;

export function useProgramEditorPaging(
  source: string, onEdit: (edit: TextEdit) => void,
  onScroll: (scroll: { left: number; top: number }) => void,
) {
  const offsets = useMemo(() => buildProgramEditorLineIndex(source), [source]);
  const [pageLine, setPageLine] = useState(1);
  const page = useMemo(() => programEditorPageAtLine(source, offsets, pageLine), [source, offsets, pageLine]);
  const [selectedSourceLine, setSelectedSourceLine] = useState(1);
  const [pendingSelection, setPendingSelection] = useState<TextSelection>();
  const globalSelection = useRef<TextSelection>({ start: 0, end: 0 });
  const projectedSelection = useRef<TextSelection | undefined>(undefined);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const select = (selection: TextSelection) => {
    globalSelection.current = selection;
    setPendingSelection(selection);
  };

  useLayoutEffect(() => {
    if (!pendingSelection) return;
    const selection = { ...normalizeTextSelection(source, pendingSelection), direction: pendingSelection.direction };
    const caret = caretOffset(selection);
    const line = sourceLineAtOffset(source, caret, offsets);
    if (line < page.firstLine || line > page.lastLine) {
      setPageLine(line);
      return;
    }
    const textarea = textareaRef.current;
    if (!textarea) return;
    const local = {
      start: sourceOffsetToPage(page, offsets, selection.start),
      end: sourceOffsetToPage(page, offsets, selection.end),
    };
    globalSelection.current = selection;
    projectedSelection.current = local;
    textarea.focus();
    textarea.setSelectionRange(local.start, local.end, selection.direction);
    const rowTop = (line - page.firstLine) * 20;
    if (rowTop < textarea.scrollTop || rowTop > textarea.scrollTop + textarea.clientHeight - 80) {
      textarea.scrollTop = Math.max(0, rowTop - 80);
    }
    onScroll({ left: textarea.scrollLeft, top: textarea.scrollTop });
    setSelectedSourceLine(line);
    setPendingSelection(undefined);
  }, [source, offsets, page, pendingSelection]);

  const updateCaret = (pointer = false) => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    const { selectionStart: start, selectionEnd: end } = textarea;
    if (!pointer && projectedSelection.current?.start === start && projectedSelection.current.end === end) return;
    projectedSelection.current = undefined;
    const selection = {
      start: pageOffsetToSource(page, offsets, start), end: pageOffsetToSource(page, offsets, end),
      direction: textarea.selectionDirection,
    };
    globalSelection.current = selection;
    setSelectedSourceLine(sourceLineAtOffset(source, caretOffset(selection), offsets));
  };

  const focusOffset = (offset: number, extend = false) => {
    const selection = globalSelection.current;
    const anchor = extend ? (selection.direction === "backward" ? selection.end : selection.start) : offset;
    select({ start: Math.min(anchor, offset), end: Math.max(anchor, offset), direction: offset < anchor ? "backward" : "forward" });
  };

  const focusSourceLine = (line: number) => focusOffset(sourceOffsetAtLine(
    source, Math.max(1, Math.min(offsets.length, Number.isFinite(line) ? Math.floor(line) : 1)), offsets,
  ));

  const change = (nextText: string, selection: TextSelection) => {
    const global = globalSelection.current;
    if (global.start < page.start || global.end > page.end) {
      const replacement = textChange(page.text, nextText).replacement.replace(/\n/g, page.lineEnding);
      onEdit(replaceTextSelection(source, global, replacement));
    } else {
      onEdit(editProgramEditorPage(source, offsets, page, nextText, selection));
    }
  };

  const keyDown = (event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    const modifier = event.metaKey || event.ctrlKey;
    const selection = globalSelection.current;
    const caret = caretOffset(selection);
    const line = sourceLineAtOffset(source, caret, offsets);
    const move = (offset: number) => { event.preventDefault(); focusOffset(offset, event.shiftKey); return true; };
    if (modifier && event.key.toLowerCase() === "a") {
      event.preventDefault(); select({ start: 0, end: source.length }); return true;
    }
    if (modifier && (event.key === "Home" || (event.metaKey && event.key === "ArrowUp"))) return move(0);
    if (modifier && (event.key === "End" || (event.metaKey && event.key === "ArrowDown"))) return move(source.length);
    if (!modifier && event.key === "Home") return move(offsets[line - 1]);
    if (!modifier && event.key === "End") {
      let end = line < offsets.length ? offsets[line] - 1 : source.length;
      if (source[end - 1] === "\r") end -= 1;
      return move(end);
    }
    if (event.key === "Backspace" || event.key === "Delete") {
      let span: TextSelection | undefined;
      if (selection.start < page.start || selection.end > page.end) span = selection;
      else if (selection.start === selection.end && event.key === "Backspace" && caret === page.start && caret > 0) {
        span = { start: caret - (source.slice(caret - 2, caret) === "\r\n" ? 2 : 1), end: caret };
      } else if (selection.start === selection.end && event.key === "Delete" && caret === page.end && caret < source.length) {
        span = { start: caret, end: caret + (source.slice(caret, caret + 2) === "\r\n" ? 2 : 1) };
      }
      if (span) { event.preventDefault(); onEdit(replaceTextSelection(source, span, "")); return true; }
    }
    if (!modifier && event.key === "ArrowLeft" && caret === page.start && caret > 0) {
      return move(caret - (source.slice(caret - 2, caret) === "\r\n" ? 2 : 1));
    }
    if (!modifier && event.key === "ArrowRight" && caret === page.end && caret < source.length) {
      return move(caret + (source.slice(caret, caret + 2) === "\r\n" ? 2 : 1));
    }
    const adjacent = event.key === "ArrowUp" && line === page.firstLine && line > 1 ? line - 1
      : event.key === "ArrowDown" && line === page.lastLine && line < offsets.length ? line + 1 : undefined;
    if (!modifier && adjacent !== undefined) {
      const start = offsets[adjacent - 1];
      let end = adjacent < offsets.length ? offsets[adjacent] - 1 : source.length;
      if (source[end - 1] === "\r") end -= 1;
      return move(Math.min(end, start + caret - offsets[line - 1]));
    }
    return false;
  };

  return { page, offsets, textareaRef, selectedSourceLine, selection: () => globalSelection.current,
    select, updateCaret, focusSourceLine, change, keyDown };
}
