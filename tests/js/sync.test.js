import { jest } from '@jest/globals';

const syncQueueCollection = {
  _toArrayResult: [],
  _countResult: 0,
  where: jest.fn(() => ({
    equals: jest.fn(() => ({
      toArray: jest.fn(async () => syncQueueCollection._toArrayResult),
      count: jest.fn(async () => syncQueueCollection._countResult),
    })),
  })),
  add: jest.fn(),
  delete: jest.fn(),
  bulkDelete: jest.fn(),
};

const mockDb = {
  sync_queue: syncQueueCollection,
  donations: {
    get: jest.fn(),
    update: jest.fn(),
    delete: jest.fn(),
    put: jest.fn(),
  },
  receipts: {
    get: jest.fn(),
    update: jest.fn(),
  },
  charities: {
    get: jest.fn(),
  },
};

const currentUserState = {
  user: {
    id: 'user-1',
    name: 'Casey',
    email: 'casey@example.com',
    filing_status: 'single',
    agi: 82000,
    marginal_tax_rate: 0.22,
    itemize_deductions: true,
  },
};

const getCurrentUser = jest.fn(() => currentUserState.user);
const getCurrentUserId = jest.fn(() => currentUserState.user?.id || null);
const setCurrentUser = jest.fn((profile) => {
  currentUserState.user = profile;
});
const apiJson = jest.fn();

// Mock the actual modules from static/js by path relative to this test file
jest.unstable_mockModule('../../static/js/db.js', () => ({ default: mockDb }));
jest.unstable_mockModule('../../static/js/services/current-user.js', () => ({
  getCurrentUser,
  getCurrentUserId,
  setCurrentUser,
}));
jest.unstable_mockModule('../../static/js/services/http.js', () => ({ apiJson }));

const { Sync } = await import('../../static/js/sync.js');

function createLocalStorageMock() {
  const store = new Map();
  return {
    getItem: jest.fn((key) => (store.has(key) ? store.get(key) : null)),
    setItem: jest.fn((key, value) => {
      store.set(key, String(value));
    }),
    removeItem: jest.fn((key) => {
      store.delete(key);
    }),
    clear: jest.fn(() => {
      store.clear();
    }),
  };
}

describe('Sync profile updates', () => {
  let consoleWarnSpy;

  beforeEach(() => {
    global.localStorage = createLocalStorageMock();
    global.window = { dispatchEvent: jest.fn() };
    global.CustomEvent = class {
      constructor(type) {
        this.type = type;
      }
    };

    currentUserState.user = {
      id: 'user-1',
      name: 'Casey',
      email: 'casey@example.com',
      filing_status: 'single',
      agi: 82000,
      marginal_tax_rate: 0.22,
      itemize_deductions: true,
    };

    jest.clearAllMocks();
    syncQueueCollection._toArrayResult = [];
    syncQueueCollection._countResult = 0;
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleWarnSpy.mockRestore();
    delete global.localStorage;
    delete global.window;
    delete global.CustomEvent;
  });

  test('queueProfileUpdate stores a pending profile payload and counts it', async () => {
    apiJson.mockResolvedValue({
      res: { ok: false, status: 503 },
      data: 'Service unavailable',
    });

    await Sync.queueProfileUpdate('user-1', {
      ...currentUserState.user,
      name: 'Casey Updated',
      provider: 'local',
    });

    expect(global.localStorage.setItem).toHaveBeenCalledWith(
      'pending_profile:user-1',
      JSON.stringify({
        name: 'Casey Updated',
        email: 'casey@example.com',
        filing_status: 'single',
        agi: 82000,
        marginal_tax_rate: 0.22,
        itemize_deductions: true,
      })
    );
    await expect(Sync.countPendingChanges('user-1')).resolves.toBe(1);
  });

  test('pushChanges syncs pending profile updates before queued records', async () => {
    global.localStorage.setItem(
      'pending_profile:user-1',
      JSON.stringify({
        name: 'Casey Synced',
        email: 'casey@example.com',
        filing_status: 'single',
        agi: 90000,
        marginal_tax_rate: 0.24,
        itemize_deductions: false,
      })
    );
    apiJson.mockResolvedValue({
      res: { ok: true, status: 200 },
      data: {
        id: 'user-1',
        name: 'Casey Synced',
        email: 'casey@example.com',
      },
    });

    await Sync.pushChanges();

    expect(apiJson).toHaveBeenCalledWith(
      '/api/me',
      expect.objectContaining({
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
      })
    );
    expect(global.localStorage.removeItem).toHaveBeenCalledWith('pending_profile:user-1');
    expect(setCurrentUser).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'user-1',
        name: 'Casey Synced',
      })
    );
  });

  test('queueAction rejects create actions without an item id', async () => {
    await Sync.queueAction(
      'donations',
      {
        user_id: 'user-1',
        charity_id: 'charity-1',
        amount: 25,
      },
      'create'
    );

    expect(mockDb.donations.put).not.toHaveBeenCalled();
    expect(syncQueueCollection.add).not.toHaveBeenCalled();
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      'Skipping sync queue action with missing item id',
      'donations',
      'create',
      expect.objectContaining({ charity_id: 'charity-1' })
    );
  });

  test('pushChanges drops malformed queued tasks with missing item ids', async () => {
    syncQueueCollection._toArrayResult = [
      {
        id: 12,
        user_id: 'user-1',
        table: 'donations',
        item_id: undefined,
        action: 'create',
        timestamp: Date.now(),
      },
    ];

    await Sync.pushChanges();

    expect(syncQueueCollection.bulkDelete).toHaveBeenCalledWith([12]);
    expect(apiJson).not.toHaveBeenCalledWith('/api/sync/batch', expect.anything());
  });
});
