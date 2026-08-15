"use client";

import { Component, type ReactNode } from "react";

interface ThemeErrorBoundaryProps {
  children: ReactNode;
  contributionId: string;
  fallback: ReactNode;
  onError?: (error: Error, contributionId: string) => void;
}

interface ThemeErrorBoundaryState {
  error: Error | null;
}

export class ThemeErrorBoundary extends Component<
  ThemeErrorBoundaryProps,
  ThemeErrorBoundaryState
> {
  state: ThemeErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ThemeErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error): void {
    this.props.onError?.(error, this.props.contributionId);
  }

  render(): ReactNode {
    return this.state.error ? this.props.fallback : this.props.children;
  }
}
