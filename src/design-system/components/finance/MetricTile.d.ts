import * as React from "react";

/**
 * Headline KPI tile for dashboards — money value in tabular mono with a
 * signed delta and optional sparkline.
 * @startingPoint section="Finance" subtitle="MetricTile — KPI value + delta" viewport="320x160"
 */
export interface MetricTileProps {
  /** Short metric name, e.g. "Saldo do mês". Rendered uppercase. */
  label?: string;
  /** Pre-formatted money string, e.g. "R$ 4.820,00". Rendered as-is in tabular mono. */
  value?: string;
  /** Optional 15×15 icon node placed at the right of the header row. */
  icon?: React.ReactNode;
  /** Change figure, e.g. "+12,4%" or "R$ 320". */
  delta?: string | null;
  /** Direction of delta — drives icon + color. "neutral" shows a minus icon in muted color. */
  deltaDir?: "up" | "down" | "neutral";
  /** Context line shown beside the delta, e.g. "vs. mês anterior". */
  sublabel?: string;
  /** Array of numeric values for a mini SVG polyline sparkline. */
  spark?: number[] | null;
  className?: string;
}

/**
 * Headline KPI tile for dashboards — money value in tabular mono with a
 * signed delta and optional SVG polyline sparkline.
 */
export function MetricTile(props: MetricTileProps): JSX.Element;
