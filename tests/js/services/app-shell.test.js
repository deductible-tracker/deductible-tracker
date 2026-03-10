import { jest } from '@jest/globals';
import { restoreAppShellIfMissingRouteContent } from '../../../static/js/services/app-shell.js';

describe('app shell restore after login', () => {
  test('restores workspace overview shell when route content is missing', () => {
    const doc = {
      querySelector: jest.fn(() => null),
      body: { classList: { add: jest.fn() } },
    };

    const changed = restoreAppShellIfMissingRouteContent(doc, '<div></div>');
    expect(changed).toBe(true);
    expect(doc.innerHTML).toBe('<div></div>');
  });

  test('does not overwrite shell when route content already exists', () => {
    const node = { innerHTML: 'content' };
    const doc = {
      querySelector: jest.fn(() => node),
      body: { classList: { add: jest.fn() } },
    };

    const changed = restoreAppShellIfMissingRouteContent(doc, '<div></div>');
    expect(changed).toBe(false);
  });
});
