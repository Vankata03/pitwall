/** The short unofficial line (spec section 15): chrome on every screen, and the Notice tile. */
export const SHORT_LINE = 'Unofficial. Not associated with the Formula 1 companies or EA.';

/** The full notice lives on the status page. */
export const FULL_NOTICE_HREF = '/status#notice';

export function ShortLine({ className }: { className?: string }) {
  return (
    <a className={className ? `pw-short-line ${className}` : 'pw-short-line'} href={FULL_NOTICE_HREF}>
      {SHORT_LINE}
    </a>
  );
}
