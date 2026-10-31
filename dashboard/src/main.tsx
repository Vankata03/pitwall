import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { RuntimeProvider } from './runtime/react';
import { createRuntime } from './runtime/runtime';
import './runtime/runtime.css';

const runtime = createRuntime({ ownBuildId: __PITWALL_BUILD_ID__ });
runtime.start();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RuntimeProvider runtime={runtime}>
      <App />
    </RuntimeProvider>
  </StrictMode>,
);
