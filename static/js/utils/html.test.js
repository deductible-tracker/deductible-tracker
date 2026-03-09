import { escapeHtml } from './html.js';

describe('escapeHtml', () => {
    test('escapes html-significant characters', () => {
        expect(escapeHtml('<div class="x">A&B\'s</div>')).toBe('&lt;div class=&quot;x&quot;&gt;A&amp;B&#039;s&lt;/div&gt;');
    });

    test('returns empty string for nullish values', () => {
        expect(escapeHtml(null)).toBe('');
        expect(escapeHtml(undefined)).toBe('');
    });
});
