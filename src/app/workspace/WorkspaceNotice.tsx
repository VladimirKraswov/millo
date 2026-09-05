import { AlertCircle, ScrollText, X } from "lucide-react";

export function WorkspaceNotice({ message, onDismiss, onLog }: {
  message?: string;
  onDismiss: () => void;
  onLog: () => void;
}) {
  if (!message) return null;
  return (
    <aside className="workspace-notice" role="alert">
      <AlertCircle aria-hidden="true" size={19} />
      <div>
        <strong>Не удалось выполнить действие</strong>
        <p>{message}</p>
      </div>
      <button onClick={onLog} type="button" title="Открыть журнал">
        <ScrollText aria-hidden="true" size={15} />Журнал
      </button>
      <button onClick={onDismiss} type="button" aria-label="Закрыть уведомление" title="Закрыть">
        <X aria-hidden="true" size={17} />
      </button>
    </aside>
  );
}
