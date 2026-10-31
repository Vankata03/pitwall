import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';

const css = readFileSync(join(import.meta.dirname, 'car-status.css'), 'utf8');

test('wheel readouts sit at fixed positions: rear above front, the driver’s left on the image’s right', () => {
  // With the car, in the centre column.
  expect(css).toContain("'rr car rl' 'fr car fl' 'frw car flw'");
  // Without the car (compact), the same arrangement.
  expect(css).toContain("'rr rl' 'fr fl' 'frw flw'");
  for (const slot of ['rr', 'rl', 'fr', 'fl', 'frw', 'flw']) expect(css).toContain(`[data-slot='${slot}']`);
});

test('text is never set below the 11px label and 14px value floors, and nothing is scaled', () => {
  const sizes = [...css.matchAll(/font-size:\s*([^;]+);/g)].map((m) => m[1]!.trim());
  for (const size of sizes) {
    const px = /^(\d+)px$/.exec(size);
    if (px) expect(Number(px[1])).toBeGreaterThanOrEqual(11);
    else expect(size).toMatch(/^var\(--pw-text-(label|value)\)$/);
  }
  expect(css).not.toMatch(/transform:\s*scale|zoom:/);
});

test('colours come from the palette tokens, never literals', () => {
  expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
});
