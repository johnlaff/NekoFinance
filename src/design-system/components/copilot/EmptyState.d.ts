import * as React from "react";

export interface EmptyStateProps {
  /** empty = no data; loading = calm spinner; skeleton = shimmer rows; error = danger. */
  variant?: "empty" | "loading" | "skeleton" | "error";
  /** Override the default 22px icon (ignored for loading/skeleton). */
  icon?: React.ReactNode;
  title?: string;
  description?: string;
  /** A <Button> or link for the primary recovery / next action. */
  action?: React.ReactNode;
  /** Number of shimmer rows when variant="skeleton". */
  skeletonRows?: number;
  className?: string;
}

/**
 * Unified empty / loading / skeleton / error placeholder. Keeps the four
 * non-content states visually consistent across the app.
 *
 * @startingPoint section="Copilot" subtitle="EmptyState — non-content state placeholder" viewport="420x260"
 */
export function EmptyState(props: EmptyStateProps): JSX.Element;
