import { ShortLine } from '../../shared/short-line';
import { defineTileModule } from '../contract';

/** The short unofficial line as a tile, so it can appear in Stream view, which has no chrome. */
export default defineTileModule({
  declaration: {
    type: 'notice',
    name: 'Notice',
    binding: 'session-wide',
    rate: 'slow',
    minSize: { w: 4, h: 1 },
    defaultSize: { w: 6, h: 1 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 480, minHeight: 0 },
    ],
    reads: [],
  },
  Component: () => (
    <div className="pw-notice-tile">
      <ShortLine />
    </div>
  ),
});
