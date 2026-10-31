import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  build: { target: 'chrome127' },
  // The collector reports its own build ID in `hello`; a page built with another ID reloads.
  // Both sides take it from PITWALL_BUILD_ID at build time.
  define: { __PITWALL_BUILD_ID__: JSON.stringify(process.env.PITWALL_BUILD_ID ?? 'dev') },
  // The module cache keeps old `import.meta.glob` results, which hides newly added tiles.
  test: { environment: 'node', experimental: { fsModuleCache: false } },
});
