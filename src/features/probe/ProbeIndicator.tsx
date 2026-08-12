import type { ConnectionState } from "../../shared/machine";

interface ProbeIndicatorProps {
  readonly active: boolean;
  readonly connection: ConnectionState;
  readonly onClick?: () => void;
}

type ProbeIndicatorState = "triggered" | "open" | "unavailable";

const stateCopy: Record<ProbeIndicatorState, { ariaLabel: string; title: string }> = {
  triggered: {
    ariaLabel: "Щуп: контакт замкнут",
    title: "Щуп замкнут: вход P активен",
  },
  open: {
    ariaLabel: "Щуп: контакт разомкнут",
    title: "Щуп разомкнут: вход P неактивен",
  },
  unavailable: {
    ariaLabel: "Щуп: нет актуального статуса",
    title: "Щуп: подключитесь к контроллеру для чтения входа P",
  },
};

export function ProbeIndicator({ active, connection, onClick }: ProbeIndicatorProps) {
  const state: ProbeIndicatorState =
    connection !== "connected" ? "unavailable" : active ? "triggered" : "open";
  const copy = stateCopy[state];

  return (
    <button
      aria-label={copy.ariaLabel}
      className={`probe-indicator is-${state}`}
      data-state={state}
      onClick={onClick}
      type="button"
      title={copy.title}
    >
      <i aria-hidden="true" />
      <span>Щуп</span>
    </button>
  );
}
