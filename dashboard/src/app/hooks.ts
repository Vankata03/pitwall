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

/**
 * Height a grid may use without the page scrolling: from the top of `grid` to the bottom of the
 * viewport, less what sits below it in the page. 0 until measured. Banners and notices change the
 * answer, so the whole page is watched rather than the grid alone.
 */
export function useAvailableHeight(grid: RefObject<HTMLElement | null>, footer: RefObject<HTMLElement | null>, viewportHeight: number): number {
  const [height, setHeight] = useState(0);
  useLayoutEffect(() => {
    const el = grid.current;
    if (!el) return;
    const measure = () => {
      const style = getComputedStyle(el);
      const padding = (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0);
      const top = el.getBoundingClientRect().top + window.scrollY;
      setHeight(Math.max(0, viewportHeight - top - padding - (footer.current?.offsetHeight ?? 0)));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(document.body);
    return () => observer.disconnect();
  }, [grid, footer, viewportHeight]);
  return height;
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

/** CSS viewport dimensions, including changes to an OBS source size. */
export function useViewportSize(): { width: number; height: number } {
  const width = useSyncExternalStore(subscribeViewport, () => window.innerWidth, () => 1920);
  const height = useSyncExternalStore(subscribeViewport, () => window.innerHeight, () => 1080);
  return { width, height };
}
