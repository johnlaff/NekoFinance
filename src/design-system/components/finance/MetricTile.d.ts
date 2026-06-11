import * as React from "react";

/**
 * Headline KPI tile for dashboards — money value in tabular mono with a
 * signed delta and optional sparkline.
 * @startingPoint section="Finance" subtitle="MetricTile — KPI value + delta" viewport="320x140"
 */
export interface MetricTileProps {
  /** Short metric name, e.g. "Net cashflow". */
  label: string;
  /** Pre-formatted money string, e.g. "$4,820.00". Cents are dimmed automatically. */
  value: string;
  /** 15×15 leading icon. */
  icon?: React.ReactNode;
  /** Change figure, e.g. "+12.4%" or "$320". */
  delta?: string;
  /** Direction of delta — colors + arrow. */
  deltaDir?: "up" | "down" | "flat";
  /** Context line under the value, e.g. "vs. last month". */
  sublabel?: string;
  /** Array of 0–100 bar heights for a mini sparkline. */
  spark?: number[];
  className?: string;
}

/**
 * Headline KPI tile for dashboards — money value in tabular mono with a
 * signed delta and optional sparkline.
 */
export function MetricTile(props: MetricTileProps): JSX.Element;
