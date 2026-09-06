import { useCallback, useEffect, useRef, useState } from "react";
import { bindSnapshotStream } from "../../platform/state/bindSnapshotStream";
import { idleSenderSnapshot, type SenderSnapshot, type SenderStateGateway } from "../../shared/dryRun";

interface ProgramSenderOptions {
  readonly desktopRuntime: boolean;
  readonly senderGateway?: SenderStateGateway;
  readonly initialSender?: SenderSnapshot;
  readonly onError: (message: string) => void;
}

export function useProgramSender({
  desktopRuntime, senderGateway, initialSender, onError,
}: ProgramSenderOptions) {
  const [sender, setSender] = useState(initialSender ?? idleSenderSnapshot);
  const eventRevision = useRef(0);
  const latestSender = useRef(sender);
  latestSender.current = sender;

  useEffect(() => {
    if (!desktopRuntime || !senderGateway) return;
    return bindSnapshotStream({
      stream: {
        readCurrent: () => senderGateway.snapshot(),
        listen: (handler) => senderGateway.subscribe((snapshot) => {
          eventRevision.current += 1;
          latestSender.current = snapshot;
          handler(snapshot);
        }),
      },
      onSnapshot: (snapshot) => {
        latestSender.current = snapshot;
        setSender(snapshot);
      },
      onError: (reason) => onError(String(reason)),
    });
  }, [desktopRuntime, senderGateway, onError]);

  // Capture before sending: events can advance this run while its IPC reply waits.
  const captureSenderResult = useCallback(() => {
    const revision = eventRevision.current;
    return (snapshot: SenderSnapshot) =>
      revision === eventRevision.current || snapshot.runSequence > latestSender.current.runSequence
        ? snapshot : latestSender.current;
  }, []);

  return { sender, setSender, captureSenderResult };
}
