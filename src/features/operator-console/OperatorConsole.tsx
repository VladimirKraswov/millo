import {
  LoaderCircle,
  Send,
  ShieldAlert,
  ShieldCheck,
  SquareTerminal,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { executeOperatorConsole } from "../../api/controller";
import type {
  CommandCompletion,
  ControllerSnapshot,
  OperatorConsoleExchange,
} from "../../shared/machine";
import {
  consoleCommandAllowed,
  consolePolicyMessage,
  normalizeSubmittedConsoleCommand,
  safeConsoleCommands,
} from "./operatorConsoleModel";

const MAX_TRANSCRIPT_ENTRIES = 120;
const MAX_COMMAND_HISTORY = 40;

interface ConsoleEntry {
  readonly id: number;
  readonly command: string;
  readonly timestampMs: number;
  readonly state: "pending" | "completed" | "rejected";
  readonly completion?: CommandCompletion;
  readonly lines: readonly string[];
}

export interface OperatorConsoleProps {
  readonly desktopRuntime: boolean;
  readonly execute?: (command: string) => Promise<OperatorConsoleExchange>;
  readonly onClose: () => void;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly open: boolean;
  readonly safeCommandMode: boolean;
  readonly snapshot: ControllerSnapshot;
}

const retainLatest = <T,>(items: readonly T[], maximum: number): readonly T[] =>
  items.length <= maximum ? items : items.slice(items.length - maximum);

export function OperatorConsole({
  desktopRuntime,
  execute = executeOperatorConsole,
  onClose,
  onSnapshot,
  open,
  safeCommandMode,
  snapshot,
}: OperatorConsoleProps) {
  const [input, setInput] = useState("");
  const [entries, setEntries] = useState<readonly ConsoleEntry[]>([]);
  const [commandHistory, setCommandHistory] = useState<readonly string[]>([]);
  const [historyCursor, setHistoryCursor] = useState<number>();
  const nextId = useRef(1);
  const streamRef = useRef<HTMLDivElement>(null);
  const connected = snapshot.connection === "connected";
  const validCommand = consoleCommandAllowed(input, safeCommandMode);
  const pending = entries.some((entry) => entry.state === "pending");

  useEffect(() => {
    const stream = streamRef.current;
    if (stream) stream.scrollTop = stream.scrollHeight;
  }, [entries]);

  useEffect(() => {
    if (open) setHistoryCursor(undefined);
  }, [open]);

  if (!open) return null;

  const append = (entry: ConsoleEntry) =>
    setEntries((current) => retainLatest([...current, entry], MAX_TRANSCRIPT_ENTRIES));

  const complete = (id: number, update: Partial<ConsoleEntry>) =>
    setEntries((current) =>
      current.map((entry) => (entry.id === id ? { ...entry, ...update } : entry)),
    );

  const submit = async (requested: string) => {
    const command = normalizeSubmittedConsoleCommand(requested, safeCommandMode);
    const id = nextId.current++;
    const timestampMs = Date.now();

    if (!consoleCommandAllowed(command, safeCommandMode)) {
      append({
        id,
        command: command || "(пусто)",
        timestampMs,
        state: "rejected",
        lines: [consolePolicyMessage(command, safeCommandMode)],
      });
      return;
    }
    if (!desktopRuntime || !connected) {
      append({
        id,
        command,
        timestampMs,
        state: "rejected",
        lines: ["Контроллер не подключён"],
      });
      return;
    }

    append({ id, command, timestampMs, state: "pending", lines: [] });
    setCommandHistory((current) =>
      retainLatest([...current.filter((item) => item !== command), command], MAX_COMMAND_HISTORY),
    );
    setHistoryCursor(undefined);
    setInput("");

    try {
      const exchange = await execute(command);
      onSnapshot(exchange.snapshot);
      complete(id, {
        state: exchange.completion === "ok" ? "completed" : "rejected",
        completion: exchange.completion,
        lines: exchange.lines.length > 0 ? exchange.lines : [exchange.completion],
      });
    } catch (error) {
      complete(id, {
        state: "rejected",
        lines: [String(error)],
      });
    }
  };

  const recallHistory = (direction: -1 | 1) => {
    if (commandHistory.length === 0) return;
    const current = historyCursor ?? commandHistory.length;
    const next = Math.min(commandHistory.length, Math.max(0, current + direction));
    setHistoryCursor(next);
    setInput(next === commandHistory.length ? "" : commandHistory[next]);
  };

  return (
    <div className="operator-console-backdrop" role="presentation">
      <section
        aria-labelledby="operator-console-title"
        aria-modal="true"
        className="operator-console"
        role="dialog"
      >
        <header>
          <div className="operator-console-title">
            <SquareTerminal aria-hidden="true" size={19} />
            <div>
              <span>GRBL</span>
              <strong id="operator-console-title">Операторская консоль</strong>
            </div>
          </div>
          <div className="operator-console-header-actions">
            <span className={`operator-console-policy${safeCommandMode ? "" : " is-expert"}`}>
              {safeCommandMode ? (
                <ShieldCheck aria-hidden="true" size={13} />
              ) : (
                <ShieldAlert aria-hidden="true" size={13} />
              )}
              {safeCommandMode ? "Безопасный режим" : "Экспертный режим"}
            </span>
            <button
              aria-label="Очистить консоль"
              disabled={entries.length === 0 || pending}
              onClick={() => setEntries([])}
              title="Очистить"
              type="button"
            >
              <Trash2 aria-hidden="true" size={15} />
            </button>
            <button aria-label="Закрыть консоль" onClick={onClose} title="Закрыть" type="button">
              <X aria-hidden="true" size={17} />
            </button>
          </div>
        </header>

        <div className="operator-console-palette" role="group" aria-label="Безопасные запросы">
          {safeConsoleCommands.map((descriptor) => (
            <button
              disabled={pending || !desktopRuntime || !connected}
              key={descriptor.command}
              onClick={() => void submit(descriptor.command)}
              type="button"
            >
              <code>{descriptor.command}</code>
              <span>{descriptor.label}</span>
            </button>
          ))}
        </div>

        <div className="operator-console-stream" ref={streamRef} role="log">
          {entries.length === 0 ? (
            <div className="operator-console-empty">
              <SquareTerminal aria-hidden="true" size={24} />
              <strong>Нет запросов в этой сессии</strong>
            </div>
          ) : (
            entries.map((entry) => (
              <article className={`console-entry is-${entry.state}`} key={entry.id}>
                <div className="console-entry-command">
                  <time dateTime={new Date(entry.timestampMs).toISOString()}>
                    {new Date(entry.timestampMs).toLocaleTimeString("ru-RU", {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                      hour12: false,
                    })}
                  </time>
                  <code>{entry.command}</code>
                  {entry.state === "pending" ? (
                    <LoaderCircle aria-label="Выполняется" className="is-spinning" size={13} />
                  ) : (
                    <span>{entry.completion ?? "blocked"}</span>
                  )}
                </div>
                {entry.lines.map((line, index) => (
                  <pre key={`${entry.id}-${index}`}>{line}</pre>
                ))}
              </article>
            ))
          )}
        </div>

        <form
          className="operator-console-input"
          onSubmit={(event) => {
            event.preventDefault();
            void submit(input);
          }}
        >
          <div>
            <span aria-hidden="true">›</span>
            <input
              aria-label="Команда GRBL"
              autoCapitalize="characters"
              autoComplete="off"
              disabled={pending}
              maxLength={safeCommandMode ? 64 : 255}
              onChange={(event) => {
                setInput(event.target.value);
                setHistoryCursor(undefined);
              }}
              onKeyDown={(event) => {
                if (event.key === "ArrowUp") {
                  event.preventDefault();
                  recallHistory(-1);
                } else if (event.key === "ArrowDown") {
                  event.preventDefault();
                  recallHistory(1);
                }
              }}
              placeholder={safeCommandMode ? "$I" : "G0 X10"}
              spellCheck={false}
              value={input}
            />
          </div>
          <button
            aria-label="Выполнить запрос"
            className={validCommand ? "is-ready" : undefined}
            disabled={pending || !input.trim()}
            title="Выполнить запрос"
            type="submit"
          >
            <Send aria-hidden="true" size={16} />
          </button>
        </form>
        <footer className={validCommand ? (safeCommandMode ? "is-safe" : "is-expert") : undefined}>
          <i aria-hidden="true" />
          <span>
            {connected
              ? consolePolicyMessage(input, safeCommandMode)
              : "Контроллер не подключён"}
          </span>
        </footer>
      </section>
    </div>
  );
}
