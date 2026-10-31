/**
 * Stands in for a tile type this build does not know, or a setting it cannot read. The tile
 * keeps its place and its saved data: moving, resizing and saving leave `type` and
 * `settings` exactly as they were.
 */
export function UnknownTile({ type }: { type: string }) {
  return (
    <div className="pw-unknown" data-testid="unknown-tile">
      <span className="pw-label">Unknown tile</span>
      <span className="pw-value">{type}</span>
      <span className="pw-unknown-note">This version of pitwall does not have it. Its settings are kept.</span>
    </div>
  );
}
