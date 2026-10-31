import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, test } from 'vitest';

const dir = import.meta.dirname;
const css = (name: string) => readFileSync(join(dir, name), 'utf8');
const tokens = css('tokens.css');
const base = css('base.css');

/** Spec section 10, Colour. */
const PALETTE: Record<string, string> = {
  '--pw-tile-bg': '#1B1F21',
  '--pw-text': '#F3F5F6',
  '--pw-text-muted': '#9AA7AD',
  '--pw-temp-cold': '#5CA7E7',
  '--pw-temp-window': '#E8EDEF',
  '--pw-temp-hot': '#FF9C35',
  '--pw-condition-healthy': '#E8EDEF',
  '--pw-condition-mid': '#9AA7AD',
  '--pw-condition-low': '#FF9C35',
  '--pw-sector-session-best': '#C58BFF',
  '--pw-sector-personal-best': '#7ED99C',
  '--pw-sector-slower': '#F2D14B',
  '--pw-energy': '#56D9CA',
  '--pw-brake': '#F55C72',
};

function declared(name: string): string | undefined {
  return new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})\\s*;`).exec(tokens)?.[1]?.toUpperCase();
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}
const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
};

describe('palette tokens', () => {
  test.each(Object.entries(PALETTE))('%s is %s', (name, hex) => {
    expect(declared(name)).toBe(hex);
  });

  test('every palette colour clears 4.5:1 on the tile background', () => {
    for (const [name, hex] of Object.entries(PALETTE)) {
      if (name === '--pw-tile-bg') continue;
      expect(contrast(hex, PALETTE['--pw-tile-bg']!), name).toBeGreaterThanOrEqual(4.5);
    }
  });

  test('chrome text and focus colours clear 4.5:1 on the page background', () => {
    const page = declared('--pw-page-bg')!;
    for (const name of ['--pw-text', '--pw-text-muted', '--pw-accent']) {
      expect(contrast(declared(name) ?? PALETTE[name]!, page), name).toBeGreaterThanOrEqual(4.5);
    }
  });
});

describe('type rules', () => {
  test('Oxanium is the display font', () => {
    expect(tokens).toMatch(/--pw-font:[^;]*Oxanium/);
  });

  test('labels are 11px and values 14px: the floor at every density', () => {
    expect(tokens).toMatch(/--pw-text-label:\s*11px;/);
    expect(tokens).toMatch(/--pw-text-value:\s*14px;/);
  });

  test('no declaration anywhere sets text below 11px', () => {
    for (const source of [tokens, base, css('app.css')]) {
      for (const m of source.matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)) {
        expect(Number(m[1]), m[0]).toBeGreaterThanOrEqual(11);
      }
    }
  });

  test('interactive controls are at least 44px', () => {
    expect(tokens).toMatch(/--pw-hit:\s*44px;/);
  });

  test('reduced motion turns off decorative transitions', () => {
    expect(base).toMatch(/@media \(prefers-reduced-motion: reduce\)/);
  });

  test('stream-safe: the page background is a token the stream view can clear', () => {
    expect(tokens).toMatch(/--pw-page-bg:/);
  });
});
