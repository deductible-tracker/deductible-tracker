import db from './db.js';
import { getCurrentUser, getCurrentUserId, setCurrentUser } from './services/current-user.js';
import { apiJson } from './services/http.js';
import { registerVaultKey, unlockVaultKey, encryptData, decryptData, ensureVaultKey } from './services/crypto.js';

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

function normalizeQueueAction(table, action) {
  if (table === 'receipts' && action === 'attach') {
    return 'create';
  }
  return typeof action === 'string' && action.trim() ? action : null;
}

function normalizeQueueItemId(value) {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed || null;
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

    const vaultKey = await ensureVaultKey(userId);

    const batch = {
      donations: [],
      receipts: [],
    };
    const taskIds = [];
    const donationUpdates = [];
    const invalidTaskIds = [];

    for (const task of queue) {
      try {
        const action = normalizeQueueAction(task.table, task.action);
        const itemId = normalizeQueueItemId(task.item_id);

        if (!action) {
          console.warn('Dropping sync task with invalid action', task);
          invalidTaskIds.push(task.id);
          continue;
        }

        if (!itemId) {
          console.warn('Dropping sync task with missing item_id', task);
          invalidTaskIds.push(task.id);
          continue;
        }

        if (task.table === 'donations') {
          const donation = await db.donations.get(itemId);
          if (donation || action === 'delete') {
            const item = {
              action,
              id: itemId,
              date: donation ? donation.date : null,
              year: donation ? donation.year : null,
              category: donation ? donation.category : null,
              amount: donation ? donation.amount : null,
              charity_id: donation ? donation.charity_id : '',
              notes: donation ? donation.notes : null,
              updated_at: donation ? donation.updated_at : null,
            };

            if (vaultKey && donation) {
              const sensitive = {
                date: donation.date,
                category: donation.category,
                amount: donation.amount,
                notes: donation.notes,
              };
              item.is_encrypted = true;
              item.encrypted_payload = await encryptData(vaultKey, sensitive);
              // Clear plaintext fields for transport
              item.date = null;
              item.category = null;
              item.amount = null;
              item.notes = null;
            }

            batch.donations.push(item);
            taskIds.push(task.id);
            if (action !== 'delete') {
              donationUpdates.push(itemId);
            }
          }
        } else if (task.table === 'receipts' && action === 'create') {
          const receipt = await db.receipts.get(itemId);
          if (receipt && receipt.donation_id) {
            const item = {
              action: 'create',
              id: receipt.id,
              donation_id: receipt.donation_id,
              key: receipt.key,
              file_name: receipt.file_name,
              content_type: receipt.content_type,
              size: receipt.size,
            };

            if (vaultKey) {
              const sensitive = {
                file_name: receipt.file_name,
                ocr_text: receipt.ocr_text,
                ocr_date: receipt.ocr_date,
                ocr_amount: receipt.ocr_amount,
              };
              item.is_encrypted = true;
              item.encrypted_payload = await encryptData(vaultKey, sensitive);
              item.file_name = null; // Don't leak filename
            }

            batch.receipts.push(item);
            taskIds.push(task.id);
          }
        }
      } catch (e) {
        console.error('Error preparing batch task', task, e);
      }
    }

    if (invalidTaskIds.length > 0) {
      await db.sync_queue.bulkDelete(invalidTaskIds);
    }

    if (batch.donations.length === 0 && batch.receipts.length === 0) {
      if (invalidTaskIds.length > 0) {
        try {
          window.dispatchEvent(new CustomEvent('sync-queue-changed'));
        } catch (e) {
          /* ignore */
        }
      }
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
        for (let remote of data.donations) {
          try {
            if (remote.is_encrypted && remote.encrypted_payload) {
              const vaultKey = await ensureVaultKey(userId);
              if (vaultKey) {
                try {
                  const decrypted = await decryptData(vaultKey, remote.encrypted_payload);
                  remote = { ...remote, ...decrypted };
                } catch (e) {
                  console.error('Failed to decrypt remote donation', remote.id, e);
                  continue;
                }
              }
            }

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
    if (!item || typeof item !== 'object') {
      console.warn('Skipping sync queue action with invalid item', table, action, item);
      return;
    }

    const normalizedAction = normalizeQueueAction(table, action);
    if (!normalizedAction) {
      console.warn('Skipping sync queue action with invalid action', table, action, item);
      return;
    }

    const itemId = normalizeQueueItemId(item.id);
    if (!itemId) {
      console.warn(
        'Skipping sync queue action with missing item id',
        table,
        normalizedAction,
        item
      );
      return;
    }

    const userId = item.user_id || getCurrentUserId();
    if (!userId) {
      console.warn('No user id for sync queue action', table, action);
      return;
    }
    // 1. Apply to local DB immediately (Optimistic UI)
    if (normalizedAction === 'create' || normalizedAction === 'update') {
      await db.table(table).put({ ...item, id: itemId, user_id: userId, sync_status: 'pending' });
    } else if (normalizedAction === 'delete') {
      await db.table(table).delete(itemId);
    }

    // 2. Add to Queue
    await db.sync_queue.add({
      table,
      user_id: userId,
      item_id: itemId,
      action: normalizedAction,
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
