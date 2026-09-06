import type { SketchJobRequest } from "../../shared/sketch";
export interface SketchHistory {
  readonly past: readonly SketchJobRequest[];
  readonly present: SketchJobRequest;
  readonly future: readonly SketchJobRequest[];
}
export type HistoryAction =
  | { readonly type: "edit"; readonly document: SketchJobRequest }
  | { readonly type: "undo" | "redo" };
export function sketchHistory(
  state: SketchHistory,
  action: HistoryAction,
): SketchHistory {
  if (action.type === "edit") {
    if (JSON.stringify(action.document) === JSON.stringify(state.present))
      return state;
    return {
      past: [...state.past.slice(-39), state.present],
      present: action.document,
      future: [],
    };
  }
  if (action.type === "undo") {
    const previous = state.past.at(-1);
    return previous
      ? {
          past: state.past.slice(0, -1),
          present: previous,
          future: [state.present, ...state.future],
        }
      : state;
  }
  const next = state.future[0];
  return next
    ? {
        past: [...state.past, state.present],
        present: next,
        future: state.future.slice(1),
      }
    : state;
}
