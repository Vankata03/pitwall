import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test } from 'vitest';
import { FULL_NOTICE_HREF, SHORT_LINE, ShortLine } from './short-line';

test('the short line is the spec wording, word for word', () => {
  expect(SHORT_LINE).toBe('Unofficial. Not associated with the Formula 1 companies or EA.');
});

test('it links to the full notice on the status page', () => {
  expect(FULL_NOTICE_HREF).toBe('/status#notice');
  const html = renderToStaticMarkup(<ShortLine />);
  expect(html).toContain(`href="${FULL_NOTICE_HREF}"`);
  expect(html).toContain(SHORT_LINE);
});
