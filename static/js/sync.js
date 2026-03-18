import db from './db.js';
import { getCurrentUser, getCurrentUserId, setCurrentUser } from './services/current-user.js';
import { apiJson } from './services/http.js';

const API_BASE = '/api';
const PENDING_PROFILE_KEY_PREFIX = 'pending_profile:';

function getPendingProfileStorageKey(userId) {
  return `${PENDING_PROFILE_KEY_PREFIX}${userId}`;
}

function normalizeProfileUpdate(profile) {
  if (!profile || typeof profile !== 'object') return null;
  return {
    name: profile.name || '',
    email: profile.email || '',
    filing_status: profile.filing_status || 'single',
    agi: Number.isFinite(profile.agi) ? profile.agi : (profile.agi ?? null),
    marginal_tax_rate: Number.isFinite(profile.marginal_tax_rate)
      ? profile.marginal_tax_rate
      : (profile.marginal_tax_rate ?? null),
    itemize_deductions: !!profile.itemize_deductions,
  };
}

function loadPendingProfileUpdate(userId) {
  if (!userId) return null;
  try {
    const raw = localStorage.getItem(getPendingProfileStorageKey(userId));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return normalizeProfileUpdate(parsed);
  } catch (e) {
    return null;
  }
}

function savePendingProfileUpdate(userId, profile) {
  if (!userId) return;
  const normalized = normalizeProfileUpdate(profile);
  if (!normalized) return;
  try {
    localStorage.setItem(getPendingProfileStorageKey(userId), JSON.stringify(normalized));
  } catch (e) {
    /* ignore */
  }
}

function clearPendingProfileUpdate(userId) {
  if (!userId) return;
  try {
    localStorage.removeItem(getPendingProfileStorageKey(userId));
  } catch (e) {
    /* ignore */
  }
}

export const Sync = {
  async pushChanges() {
    const userId = getCurrentUserId();
    if (!userId) return;

    const pendingProfile = loadPendingProfileUpdate(userId);
    if (pendingProfile) {
      try {
        const { res, data } = await apiJson(`${API_BASE}/me`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(pendingProfile),
        });
        if (res.ok) {
          clearPendingProfileUpdate(userId);
          if (data && data.id) {
            setCurrentUser(data);
          } else {
            setCurrentUser({ ...(getCurrentUser() || {}), id: userId, ...pendingProfile });
          }
        } else {
          console.warn('Profile sync failed', res.status);
          if (res.status === 401) {
            console.warn('Unauthorized during sync. Stopping.');
            return;
          }
        }
      } catch (err) {
        console.error('Profile sync failed', err);
      }
    }

    const queue = await db.sync_queue.where('user_id').equals(userId).toArray();
    if (queue.length === 0) return;

    console.log('Pushing changes (batched)...', queue.length);

    const batch = {
      donations: [],
      receipts: [],
    };
    const taskIds = [];
    const donationUpdates = [];

    for (const task of queue) {
      try {
        if (!task.item_id && task.action !== 'delete') {
          console.warn('Skipping sync task with missing item_id', task);
          continue;
        }

        if (task.table === 'donations') {
          const donation = task.item_id ? await db.donations.get(task.item_id) : null;
          if (donation || task.action === 'delete') {
            batch.donations.push({
              action: task.action,
              id: task.item_id,
              date: donation ? donation.date : null,
              year: donation ? donation.year : null,
              category: donation ? donation.category : null,
              amount: donation ? donation.amount : null,
              charity_id: donation ? donation.charity_id : '',
              notes: donation ? donation.notes : null,
              updated_at: donation ? donation.updated_at : null,
            });
            taskIds.push(task.id);
            if (task.action !== 'delete') {
              donationUpdates.push(task.item_id);
            }
          }
        } else if (task.table === 'receipts' && task.action === 'create') {
          const receipt = await db.receipts.get(task.item_id);
          if (receipt && receipt.donation_id) {
            batch.receipts.push({
              action: 'create',
              id: receipt.id,
              donation_id: receipt.donation_id,
              key: receipt.key,
              file_name: receipt.file_name,
              content_type: receipt.content_type,
              size: receipt.size,
            });
            taskIds.push(task.id);
          }
        }
      } catch (e) {
        console.error('Error preparing batch task', task, e);
      }
    }

    if (batch.donations.length === 0 && batch.receipts.length === 0) {
      return;
    }

    try {
      const { res } = await apiJson(`${API_BASE}/sync/batch`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(batch),
      });

      if (res.ok) {
        // Success: Clear all task IDs from the queue
        await db.sync_queue.bulkDelete(taskIds);
        // Mark donations as synced
        for (const id of donationUpdates) {
          await db.donations.update(id, { sync_status: 'synced' });
        }
      } else {
        console.warn('Batch sync failed', res.status);
        if (res.status === 401) {
          console.warn('Unauthorized during sync. Stopping.');
          return;
        }
      }
    } catch (err) {
      console.error('Batch sync request failed', err);
    }

    try {
      window.dispatchEvent(new CustomEvent('sync-queue-changed'));
    } catch (e) {
      /* ignore */
    }
  },

  async pullChanges() {
    console.log('Pulling changes...');
    const userId = getCurrentUserId();
    if (!userId) return;
    const lastKey = `last_sync_${userId}`;
    const lastSync = localStorage.getItem(lastKey);
    const url = lastSync
      ? `${API_BASE}/donations?since=${encodeURIComponent(lastSync)}`
      : `${API_BASE}/donations`;
    try {
      const { res, data } = await apiJson(url);
      if (!res.ok) {
        if (res.status === 401) {
          console.warn('Unauthorized during pull');
          return;
        }
        console.warn('Pull failed', res.status);
        return;
      }
      if (data && Array.isArray(data.donations)) {
        for (const remote of data.donations) {
          try {
            if (remote.deleted) {
              await db.donations.delete(remote.id);
            } else {
              // Normalize date field if needed
              const local = {
                id: remote.id,
                user_id: remote.user_id,
                year: remote.year,
                date: remote.date,
                category: remote.category || 'money',
                amount: remote.amount ?? 0,
                charity_id: remote.charity_id,
                notes: remote.notes || null,
                sync_status: 'synced',
                updated_at: remote.updated_at || null,
                created_at: remote.created_at || null,
              };
              await db.donations.put(local);
            }
          } catch (e) {
            console.error('Failed to merge remote donation', remote.id, e);
          }
        }
      }
      // update last sync time
      localStorage.setItem(lastKey, new Date().toISOString());
      try {
        window.dispatchEvent(new CustomEvent('sync-queue-changed'));
      } catch (e) {
        /* ignore */
      }
    } catch (e) {
      console.error('Pull failed', e);
    }
  },

  async queueAction(table, item, action) {
    const userId = item.user_id || getCurrentUserId();
    if (!userId) {
      console.warn('No user id for sync queue action', table, action);
      return;
    }
    // 1. Apply to local DB immediately (Optimistic UI)
    if (action === 'create' || action === 'update') {
      await db.table(table).put({ ...item, user_id: userId, sync_status: 'pending' });
    } else if (action === 'delete') {
      await db.table(table).delete(item.id);
    }

    // 2. Add to Queue
    await db.sync_queue.add({
      table,
      user_id: userId,
      item_id: item.id,
      action,
      timestamp: Date.now(),
    });

    // 3. Trigger Sync (fire and forget)
    this.pushChanges();
    try {
      window.dispatchEvent(new CustomEvent('sync-queue-changed'));
    } catch (e) {
      /* ignore */
    }
  },

  async queueProfileUpdate(userId, profile = getCurrentUser()) {
    if (!userId) return;
    savePendingProfileUpdate(userId, profile);
    this.pushChanges();
    try {
      window.dispatchEvent(new CustomEvent('sync-queue-changed'));
    } catch (e) {
      /* ignore */
    }
  },

  async countPendingChanges(userId = getCurrentUserId()) {
    if (!userId) return 0;
    let pending = 0;
    try {
      pending = await db.sync_queue.where('user_id').equals(userId).count();
    } catch (e) {
      /* ignore */
    }
    if (loadPendingProfileUpdate(userId)) pending += 1;
    return pending;
  },
};
