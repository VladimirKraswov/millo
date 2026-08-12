import { Component, type ErrorInfo, type ReactNode } from "react";

interface PluginUiErrorBoundaryProps {
  readonly contributionId: string;
  readonly children: ReactNode;
  readonly onError?: (contributionId: string, error: unknown) => void;
}

interface PluginUiErrorBoundaryState {
  readonly failed: boolean;
}

export class PluginUiErrorBoundary extends Component<
  PluginUiErrorBoundaryProps,
  PluginUiErrorBoundaryState
> {
  state: PluginUiErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): PluginUiErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, _info: ErrorInfo): void {
    this.props.onError?.(this.props.contributionId, error);
  }

  render(): ReactNode {
    return this.state.failed ? null : this.props.children;
  }
}
