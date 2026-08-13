import type { SenderSnapshot } from "../../shared/dryRun";
import { senderHeartbeat, senderTiming } from "./dryRunReadModel";

export function SenderTiming({ sender }: { readonly sender: SenderSnapshot }) {
  const timing = senderTiming(sender);
  const heartbeat = senderHeartbeat(sender);
  return (
    <>
      <div className="sender-timing" aria-label="Время выполнения">
        <span>
          Прошло <code>{timing.elapsed}</code>
        </span>
        <span>
          {timing.estimateLabel === "ETA" ? "Осталось" : "Осталось ≥"}{" "}
          <code>{timing.remaining}</code>
        </span>
      </div>
      <div className="sender-heartbeat" aria-label="Подтверждения контроллера">
        <span>ACK #{heartbeat.sequence}</span>
        <code>
          {heartbeat.lastLine} · {heartbeat.age}
        </code>
        <strong className={heartbeat.shutdownAcknowledged ? undefined : "is-placeholder"}>
          M5 · M9 OK
        </strong>
      </div>
    </>
  );
}
