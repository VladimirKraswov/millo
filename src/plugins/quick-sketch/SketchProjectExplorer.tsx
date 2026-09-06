import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Check,
  Circle,
  Copy,
  Link2,
  LockKeyhole,
  PanelLeftClose,
  PanelLeftOpen,
  Pencil,
  Pentagon,
  Square,
  Trash2,
  X,
} from "lucide-react";
import type { SketchShape } from "../../shared/sketch";
import { operationLabels } from "./sketchModel";

export function SketchProjectExplorer({
  shapes,
  selection,
  expanded,
  cancelEditing,
  onToggle,
  onSelect,
  onRename,
  onDelete,
  onDuplicate,
  onLock,
  onEditingChange,
}: {
  readonly shapes: readonly SketchShape[];
  readonly selection: readonly string[];
  readonly expanded: boolean;
  readonly cancelEditing: number;
  readonly onToggle: () => void;
  readonly onSelect: (id?: string, additive?: boolean) => void;
  readonly onRename: (id: string, name: string) => void;
  readonly onDelete: () => void;
  readonly onDuplicate: () => void;
  readonly onLock: () => void;
  readonly onEditingChange: (editing: boolean) => void;
}) {
  const [renaming, setRenaming] = useState<{ id: string; name: string }>();
  const input = useRef<HTMLInputElement>(null);
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const selected = shapes.filter((s) => selection.includes(s.id));
  const single = selected.length === 1 ? selected[0] : undefined;
  useLayoutEffect(() => {
    onEditingChange(Boolean(renaming));
    return () => onEditingChange(false);
  }, [Boolean(renaming), onEditingChange]);
  useLayoutEffect(() => {
    if (renaming) {
      input.current?.focus();
      input.current?.select();
    }
  }, [renaming?.id]);
  useEffect(() => {
    setRenaming(undefined);
  }, [cancelEditing]);
  useEffect(() => {
    if (renaming && !shapes.some((s) => s.id === renaming.id))
      setRenaming(undefined);
  }, [shapes, renaming]);
  const finish = () => {
    if (!renaming?.name.trim()) return;
    onRename(renaming.id, renaming.name.trim());
    rows.current.get(renaming.id)?.focus();
    setRenaming(undefined);
  };
  const rename = (shape: SketchShape) => {
    setRenaming({ id: shape.id, name: shape.name });
  };
  return (
    <aside
      className={`sketch-explorer${expanded ? " is-expanded" : ""}`}
      aria-label="Обозреватель проекта"
    >
      <header>
        <button
          type="button"
          aria-label={
            expanded ? "Свернуть обозреватель" : "Показать обозреватель"
          }
          title={expanded ? "Свернуть обозреватель" : "Показать обозреватель"}
          aria-expanded={expanded}
          onClick={() => {
            setRenaming(undefined);
            onToggle();
          }}
        >
          {expanded ? (
            <PanelLeftClose size={17} />
          ) : (
            <PanelLeftOpen size={17} />
          )}
        </button>
        <strong>Фигуры</strong>
        <span>{shapes.length}</span>
      </header>
      {expanded && (
        <>
          <div
            className="sketch-explorer-actions"
            role="toolbar"
            aria-label="Действия с фигурами"
          >
            <button
              type="button"
              title="Переименовать фигуру"
              aria-label="Переименовать фигуру"
              disabled={!single}
              onClick={() => single && rename(single)}
            >
              <Pencil size={15} />
            </button>
            <button
              type="button"
              title="Создать копию фигуры"
              aria-label="Создать копию фигуры"
              disabled={!single}
              onClick={onDuplicate}
            >
              <Copy size={15} />
            </button>
            <button
              type="button"
              title="Защитить положение выбранных фигур"
              aria-label="Защитить положение выбранных фигур"
              disabled={!selected.length}
              aria-pressed={Boolean(
                selected.length && selected.every((s) => s.locked),
              )}
              onClick={onLock}
            >
              <LockKeyhole size={15} />
            </button>
            <button
              type="button"
              title="Удалить выбранные фигуры"
              aria-label="Удалить выбранные фигуры"
              disabled={!selected.length}
              onClick={onDelete}
            >
              <Trash2 size={15} />
            </button>
          </div>
          <ul
            className="sketch-project-list"
            aria-label="Фигуры и операции"
            onKeyDown={(e) => {
              if ((e.target as Element).closest("input")) return;
              if (e.key === "F2" && single) {
                e.preventDefault();
                e.stopPropagation();
                rename(single);
              }
              const index = shapes.findIndex((s) => s.id === selection.at(-1));
              const next =
                e.key === "ArrowDown"
                  ? Math.min(shapes.length - 1, index + 1)
                  : e.key === "ArrowUp"
                    ? Math.max(0, index - 1)
                    : e.key === "Home"
                      ? 0
                      : e.key === "End"
                        ? shapes.length - 1
                        : -1;
              if (next >= 0 && shapes[next]) {
                e.preventDefault();
                e.stopPropagation();
                onSelect(shapes[next].id);
                rows.current.get(shapes[next].id)?.focus();
              }
            }}
          >
            {shapes.map((shape) => {
              const Icon =
                shape.geometry.kind === "circle"
                  ? Circle
                  : shape.geometry.kind === "rectangle"
                    ? Square
                    : Pentagon;
              return (
                <li
                  key={shape.id}
                  className={`sketch-project-item is-${shape.operation.kind}${selection.includes(shape.id) ? " is-selected" : ""}`}
                >
                  {renaming?.id === shape.id ? (
                    <form
                      className="sketch-rename"
                      onSubmit={(e) => {
                        e.preventDefault();
                        finish();
                      }}
                    >
                      <input
                        ref={input}
                        aria-label="Новое название фигуры"
                        value={renaming.name}
                        maxLength={120}
                        required
                        onChange={(e) =>
                          setRenaming({ ...renaming, name: e.target.value })
                        }
                      />
                      <button
                        type="submit"
                        title="Применить название"
                        aria-label="Применить название"
                      >
                        <Check size={14} />
                      </button>
                      <button
                        type="button"
                        title="Отменить переименование"
                        aria-label="Отменить переименование"
                        onClick={() => setRenaming(undefined)}
                      >
                        <X size={14} />
                      </button>
                    </form>
                  ) : (
                    <button
                      type="button"
                      className="sketch-shape-select"
                      ref={(node) => {
                        if (node) rows.current.set(shape.id, node);
                        else rows.current.delete(shape.id);
                      }}
                      aria-pressed={selection.includes(shape.id)}
                      title={`${shape.name} · ${operationLabels[shape.operation.kind]}`}
                      onClick={(e) =>
                        onSelect(shape.id, e.shiftKey || e.metaKey || e.ctrlKey)
                      }
                      onDoubleClick={() => rename(shape)}
                    >
                      <Icon size={15} />
                      <span>
                        <strong>{shape.name}</strong>
                        <small>
                          {operationLabels[shape.operation.kind]} ·{" "}
                          {shape.operation.through
                            ? "насквозь"
                            : `${shape.operation.depthMm} мм`}
                        </small>
                      </span>
                      <i>
                        {(shape.constraints?.x || shape.constraints?.y) && (
                          <Link2 size={11} />
                        )}
                        {shape.locked && <LockKeyhole size={11} />}
                      </i>
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
          {!shapes.length && <p className="sketch-explorer-empty">Нет фигур</p>}
        </>
      )}
    </aside>
  );
}
