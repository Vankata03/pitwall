import type { Density } from '../contract';

/** Tile padding from `frame-tiles.css`, the label line and the gap between parts. */
const PAD_X = 32;
const PAD_Y = 24;
const LABEL = 16;
const GAP = 8;
/** Width of the longest headline ("+123.456") in ems of the tile font, with tabular digits. */
const HEADLINE_EMS = 5.2;
const MIN_FONT = 36;
const MAX_FONT = 168;
/** A graph shorter or narrower than this cannot be read, so the tile shows the headline alone. */
const MIN_CHART_HEIGHT = 60;
const MIN_CHART_WIDTH = 160;

export interface DeltaGeometry {
  /** Headline beside the graph, above it, or alone. */
  arrangement: 'headline' | 'stacked' | 'side';
  /** Headline font size in CSS px. Chosen from the tile box, never from the density alone. */
  fontSize: number;
  /** Graph box in CSS px, or null when the density or the box leaves no room. */
  chart: { width: number; height: number } | null;
}

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));

/**
 * The tile box decides everything: the headline is as large as the box allows at every
 * density, and the graph takes what is left. Pure, so the sizes can be tested without a DOM.
 */
export function deltaGeometry(width: number, height: number, density: Density): DeltaGeometry {
  const w = Math.max(0, Math.floor(width) - PAD_X);
  const h = Math.max(0, Math.floor(height) - PAD_Y);
  const alone = (): DeltaGeometry => ({
    arrangement: 'headline',
    fontSize: clamp(Math.floor(Math.min(w / HEADLINE_EMS, h - LABEL - GAP)), MIN_FONT, MAX_FONT),
    chart: null,
  });
  if (density === 'compact') return alone();

  if (w >= 880 && w >= 2.2 * h) {
    const fontSize = clamp(Math.floor(Math.min((w * 0.36) / HEADLINE_EMS, h - LABEL - GAP)), MIN_FONT, MAX_FONT);
    const chartWidth = w - Math.ceil(fontSize * HEADLINE_EMS) - 3 * GAP;
    return { arrangement: 'side', fontSize, chart: chartWidth >= MIN_CHART_WIDTH && h >= MIN_CHART_HEIGHT ? { width: chartWidth, height: h } : null };
  }

  const fontSize = clamp(Math.floor(Math.min(w / HEADLINE_EMS, (h - LABEL - 2 * GAP) * 0.42)), MIN_FONT, MAX_FONT);
  const chartHeight = h - LABEL - fontSize - 2 * GAP;
  if (chartHeight < MIN_CHART_HEIGHT || w < MIN_CHART_WIDTH) return alone();
  return { arrangement: 'stacked', fontSize, chart: { width: w, height: chartHeight } };
}
