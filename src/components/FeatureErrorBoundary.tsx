import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";
import { reportUiError } from "../api/audit";

interface Props {
  name: string;
  children: ReactNode;
  onError?: (message: string) => void;
  fallback?: ReactNode;
}

export class FeatureErrorBoundary extends Component<Props, { error?: string }> {
  state: { error?: string } = {};

  static getDerivedStateFromError(error: unknown) {
    return { error: String(error) };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    reportUiError(this.props.name, error, info.componentStack ?? "");
    this.props.onError?.(`${this.props.name}: ${String(error)}`);
  }

  render() {
    if (!this.state.error) return this.props.children;
    if (this.props.fallback) return this.props.fallback;
    return (
      <section className="feature-failure" role="alert">
        <AlertTriangle aria-hidden="true" size={24} />
        <h2>Не удалось показать {this.props.name.toLocaleLowerCase("ru")}</h2>
        <p>Управление станком остаётся в верхней панели. Сбой записан в журнал.</p>
        <details>
          <summary>Подробности</summary>
          <pre>{this.state.error}</pre>
        </details>
        <button onClick={() => this.setState({ error: undefined })} type="button">
          <RotateCcw aria-hidden="true" size={16} />
          Восстановить панель
        </button>
      </section>
    );
  }
}
