import { useEffect, useMemo, useState } from 'react';
import type { Device } from '../device/device';
import { arrangementOf, type Tile } from '../layout/model';
import type { LayoutController } from '../layout/controller';
import { Banners, SlowDrawNotice, useSlow } from '../runtime/react';
import {
  carSettingOf,
  layoutsRevisionOf,
  participantsOf,
  playerCarOf,
  resolveBinding,
  resolveFocus,
  type ResolvedCar,
} from '../session/cars';
import { ShortLine } from '../shared/short-line';
import type { TileRegistry } from '../tiles/registry';
import { TowerFocusProvider } from '../tiles/types/timing-tower';
import { AddTile } from './add-tile';
import { DisplayMenu } from './display-menu';
import { FocusPicker } from './focus-picker';
import { DashboardGrid } from './grid';
import { useControllerState, useDeviceSettings, useElementWidth, useScreenClass } from './hooks';
import { LayoutSwitcher } from './layout-switcher';
import { Notices } from './notices';
import { TileSettingsDialog } from './tile-settings';

export interface DashboardServices {
  controller: LayoutController;
  device: Device;
  registry: TileRegistry;
}

/** Width the grid is laid out at before it has been measured. */
const FALLBACK_WIDTH = 1264;

export function Dashboard({ controller, device, registry }: DashboardServices) {
  const state = useControllerState(controller);
  const settings = useDeviceSettings(device);
  const viewportClass = useScreenClass();
  const wire = useSlow((s) => s.state);
  const participants = useMemo(() => participantsOf(wire), [wire]);
  const playerCar = playerCarOf(wire);
  const revision = layoutsRevisionOf(wire);
  const [gridRef, measured] = useElementWidth<HTMLElement>();
  const [settingsFor, setSettingsFor] = useState<string | null>(null);

  useEffect(() => {
    if (revision !== undefined) void controller.revisionChanged(revision);
  }, [controller, revision]);

  const { edit, current } = state;
  const screenClass = edit?.screenClass ?? viewportClass;
  const shown = edit?.draft ?? current;
  const tiles = shown ? arrangementOf(shown, screenClass) : [];
  const editedTile = settingsFor ? tiles.find((t) => t.i === settingsFor) : undefined;
  const editedModule = editedTile ? registry.get(editedTile.type) : undefined;

  const carFor = (tile: Tile): ResolvedCar | null => {
    if (registry.get(tile.type)?.declaration.binding !== 'follows-car') return null;
    return resolveBinding(carSettingOf(tile.settings), { participants, playerCar, focus: settings.focus });
  };

  const focused = settings.focus ? resolveFocus({ participants, playerCar, focus: settings.focus }) : undefined;
  const towerFocus = !focused ? undefined : 'index' in focused ? focused.index : null;

  return (
    <div className="pw-app" data-editing={edit ? 'true' : undefined}>
      <header className="pw-chrome">
        <h1 className="pw-wordmark">pitwall</h1>
        <div className="pw-chrome-controls">
          {current ? (
            <LayoutSwitcher
              layouts={state.layouts}
              current={edit?.targetName ?? current.name}
              disabled={edit !== null}
              onSelect={(name) => void controller.select(name)}
              onDuplicate={(name) => void controller.duplicate(name)}
              onRename={(name, next) => void controller.rename(name, next)}
              onDelete={(name) => void controller.remove(name)}
            />
          ) : null}
          <FocusPicker participants={participants} focus={settings.focus} onChange={(identity) => device.setFocus(identity)} />
          <DisplayMenu units={settings.units} onChange={(units) => device.setUnits(units)}>
            <SlowDrawNotice />
          </DisplayMenu>
          {edit ? (
            <>
              <AddTile registry={registry} onAdd={(type, size) => controller.addTile(type, size)} />
              <button type="button" className="pw-button pw-primary" disabled={edit.saving} onClick={() => void controller.done()}>
                {edit.saving ? 'Saving' : 'Done'}
              </button>
              <button type="button" className="pw-button" disabled={edit.saving} onClick={() => controller.cancelEdit()}>
                Cancel
              </button>
            </>
          ) : (
            <button type="button" className="pw-button" disabled={!current} onClick={() => controller.startEdit(viewportClass)}>
              Edit
            </button>
          )}
          <a className="pw-button pw-link-button" href="/status">
            Status
          </a>
        </div>
      </header>

      <Banners />
      {edit ? (
        <div className="pw-edit-status" role="status">
          <span>Editing the {edit.screenClass} arrangement.</span>
          {edit.base.preset ? (
            <span>
              {edit.base.name} is read-only.{' '}
              {edit.dirty ? `Done saves your changes as “${edit.targetName}”.` : 'Your first change will be saved as a copy.'}
            </span>
          ) : null}
          {edit.error ? (
            <span className="pw-edit-error" role="alert">
              Could not save: {edit.error}
            </span>
          ) : null}
        </div>
      ) : null}
      <Notices notices={state.notices} onDismiss={(id) => controller.dismissNotice(id)} />

      <main className="pw-main" ref={gridRef}>
        {state.status === 'error' ? (
          <div className="pw-load-error" role="alert">
            <p>Could not load layouts from the collector.</p>
            <button type="button" className="pw-button" onClick={() => void controller.start()}>
              Try again
            </button>
          </div>
        ) : null}
        {state.status === 'loading' ? <p className="pw-label">Loading layout</p> : null}
        <TowerFocusProvider value={towerFocus}>
          {shown ? (
            <DashboardGrid
              tiles={tiles}
              screenClass={screenClass}
              width={measured > 0 ? measured : FALLBACK_WIDTH}
              editing={edit !== null}
              registry={registry}
              carFor={carFor}
              units={settings.units}
              onMoveResize={(items) => controller.moveResize(items)}
              onOpenSettings={(tile) => setSettingsFor(tile.i)}
              onRemove={(tile) => controller.removeTile(tile.i)}
            />
          ) : null}
        </TowerFocusProvider>
      </main>

      <footer className="pw-footer">
        <ShortLine />
      </footer>

      {editedTile && editedModule ? (
        <TileSettingsDialog
          tile={editedTile}
          module={editedModule}
          participants={participants}
          onChange={(next) => controller.setTileSettings(editedTile.i, next)}
          onClose={() => setSettingsFor(null)}
        />
      ) : null}
    </div>
  );
}
