import { jest } from '@jest/globals';
import { apiJson } from '../../../static/js/services/http.js';

describe('apiJson', () => {
  beforeEach(() => {
    global.document = { cookie: '' };
    global.fetch = jest.fn(async (_path, options) => ({
      ok: true,
      status: 200,
      headers: {
        get: () => 'application/json',
      },
      json: async () => ({ ok: true, headers: options.headers || {} }),
      text: async () => '',
    }));
  });

  afterEach(() => {
    delete global.document;
    delete global.fetch;
  });

  test('does not require csrf cookie for non-api post requests', async () => {
    await expect(apiJson('/auth/logout', { method: 'POST' })).resolves.toMatchObject({
      res: expect.objectContaining({ ok: true }),
    });

    expect(global.fetch).toHaveBeenCalledWith(
      '/auth/logout',
      expect.objectContaining({
        credentials: 'include',
        headers: {},
      })
    );
  });

  test('adds csrf header for api mutation requests when auth cookie exists', async () => {
    global.document.cookie = 'theme=dark; auth_token=csrf-123';

    await apiJson('/api/me', { method: 'PUT' });

    expect(global.fetch).toHaveBeenCalledWith(
      '/api/me',
      expect.objectContaining({
        headers: expect.objectContaining({
          'X-CSRF-Token': 'csrf-123',
        }),
      })
    );
  });

  test('still sends api mutation requests when csrf cookie is missing', async () => {
    await expect(apiJson('/api/me', { method: 'PUT' })).resolves.toMatchObject({
      res: expect.objectContaining({ ok: true }),
    });

    expect(global.fetch).toHaveBeenCalledWith(
      '/api/me',
      expect.objectContaining({
        headers: {},
      })
    );
  });
});
