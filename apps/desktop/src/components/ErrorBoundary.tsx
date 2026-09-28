import { Component, type ErrorInfo, type ReactNode } from "react";

/**
 * Keeps an error inside one part of the screen (a panel, an editor) so the
 * rest of the app keeps working. `fallback` is shown with the error.
 */
export class ErrorBoundary extends Component<
  { fallback: (error: Error) => ReactNode; children: ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  render() {
    return this.state.error ? this.props.fallback(this.state.error) : this.props.children;
  }
}
