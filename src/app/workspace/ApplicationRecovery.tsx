import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { AlertTriangle, RotateCcw, Download } from "lucide-react";
import { isDesktopRuntime } from "../../api/controller";
import { exportDiagnosticLog } from "../../api/audit";
import { RealtimeControls } from "../../features/machine-control/RealtimeControls";
import { bindMachineStateStream } from "../../platform/machine/MachineStateEventStream";
import { tauriMachineStateEventStream } from "../../platform/machine/tauriMachineStateEventStream";
import { MachineSnapshotStore } from "../../platform/machine/MachineStateSource";
import { emptySnapshot } from "../../shared/machine";

export function ApplicationRecovery() {
  const store = useMemo(() => new MachineSnapshotStore(emptySnapshot), []);
  const snapshot = useSyncExternalStore(store.subscribe, store.current);
  const [error, setError] = useState<string>();
  const desktop = isDesktopRuntime();
  useEffect(() => {
    if (!desktop) return;
    return bindMachineStateStream({ stream: tauriMachineStateEventStream, store, onError: (failure) => setError(String(failure)) });
  }, [desktop, store]);
  const exportLog = async () => {
    try { await exportDiagnosticLog("text"); } catch (failure) { setError(String(failure)); }
  };
  return <main className="application-recovery">
    <header><strong>Millo</strong><RealtimeControls desktopRuntime={desktop} snapshot={snapshot} onSnapshot={store.publish} onError={setError} /></header>
    <section><AlertTriangle size={32} /><h1>Интерфейс требует восстановления</h1>
      <p>Контроллер: {snapshot.connection === "connected" ? snapshot.machine.reportedMode : "нет связи"}. Ошибка интерфейса сама по себе не останавливает станок.</p>
      <p>При необходимости используйте паузу или Reset вверху. Восстановление интерфейса не запускает задание заново.</p>
      {error && <p role="alert">{error}</p>}
      <div><button onClick={() => window.location.reload()} type="button"><RotateCcw size={16} />Восстановить интерфейс</button><button disabled={!desktop} onClick={() => void exportLog()} type="button"><Download size={16} />Сохранить журнал</button></div>
    </section>
  </main>;
}
