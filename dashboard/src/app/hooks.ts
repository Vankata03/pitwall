import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type RefObject } from 'react';
import type { Device, DeviceSettings } from '../device/device';
import type { ControllerState, LayoutController } from '../layout/controller';
import { screenClassOf, type ScreenClass } from '../layout/screen';

export function useControllerState(controller: LayoutController): ControllerState {
  return useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);
}

export function useDeviceSettings(device: Device): DeviceSettings {
  return useSyncExternalStore(device.subscribe, device.getSnapshot, device.getSnapshot);
}

function subscribeViewport(listener: () => void): () => void {
  window.addEventListener('resize', listener);
  return () => window.removeEventListener('resize', listener);
}

/** The screen class of the viewport: the width rule picks the arrangement. */
export function useScreenClass(): ScreenClass {
  return useSyncExternalStore(
    subscribeViewport,
    () => screenClassOf(window.innerWidth),
    () => 'wide',
  );
}

/** Content width of an element, kept current with a ResizeObserver. 0 until measured. */
export function useElementWidth<T extends HTMLElement>(): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => setWidth(el.clientWidth));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, width];
}

/** Run `onEscape` when Escape is pressed while `active`. */
export function useEscape(active: boolean, onEscape: () => void): void {
  const latest = useRef(onEscape);
  latest.current = onEscape;
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        latest.current();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [active]);
}
