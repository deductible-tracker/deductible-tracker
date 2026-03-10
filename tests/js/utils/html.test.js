import { escapeHtml } from '../../../static/js/utils/html.js';

describe('escapeHtml', () => {
  test('escapes html-significant characters', () => {
    expect(escapeHtml('<div>"&</div>')).toBe('&lt;div&gt;&quot;&amp;&lt;/div&gt;');
  });

  test('returns empty string for nullish values', () => {
    expect(escapeHtml(null)).toBe('');
    expect(escapeHtml(undefined)).toBe('');
  });
});
