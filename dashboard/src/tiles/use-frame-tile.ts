import { useEffect, useRef, type DependencyList } from 'react';
import type { DrawContext } from '../runtime/loop';
import { useRuntime } from '../runtime/react';
import type { TileDeclaration } from '../runtime/tile';
import type { ResolvedCar } from '../session/cars';

/**
 * Register a `frame` tile with the shared render loop for as long as it is mounted.
 * `draw` runs on animation frames when the tile's data changed; write straight into the DOM
 * (see `writeReadout`) and never into React state. `deps` lists what `draw` closes over
 * besides the car, such as density, size or settings: a change redraws once.
 */
export function useFrameTile(
  declaration: TileDeclaration,
  car: ResolvedCar | null,
  draw: (ctx: DrawContext) => void,
  deps: DependencyList,
): void {
  const { loop } = useRuntime();
  const latest = useRef({ car, draw });
  latest.current = { car, draw };
  const handle = useRef<ReturnType<typeof loop.register> | null>(null);

  useEffect(() => {
    handle.current = loop.register({
      declaration,
      car: () => {
        const c = latest.current.car;
        return c && (c.status === 'ok' || c.status === 'retired') ? c.index : null;
      },
      draw: (ctx) => latest.current.draw(ctx),
    });
    return () => {
      handle.current?.unregister();
      handle.current = null;
    };
  }, [loop, declaration]);

  const carIndex = car && (car.status === 'ok' || car.status === 'retired') ? car.index : null;
  useEffect(() => {
    handle.current?.invalidate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [carIndex, ...deps]);
}
