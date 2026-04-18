/**
 * Data cache management: refreshing donations, receipts, and
 * charities from the server into the local IndexedDB.
 */

import db from '../db.js';
import { apiJson } from './http.js';
import { getCurrentUserId } from './current-user.js';
import { fetchCharitiesFromServer } from './api-client.js';

export async function clearUserCaches() {
  try {
    await db.donations.clear();
  } catch (e) {
    /* ignore */
  }
  try {
    await db.receipts.clear();
  } catch (e) {
    /* ignore */
  }
  try {
    await db.charities.clear();
  } catch (e) {
    /* ignore */
  }
  // NOTE: do not clear `sync_queue` here so pending offline changes are not lost on logout
}

export async function refreshDonationsFromServer() {
  const userId = getCurrentUserId();
  if (!userId) return;
  const { res, data } = await apiJson('/api/donations');
  if (!res.ok || !data || !data.donations) return;
  const donations = data.donations.map((d) => ({
    id: d.id,
    user_id: userId,
    year: d.year,
    date: d.date,
    category: d.category || 'money',
    amount: d.amount ?? 0,
    charity_id: d.charity_id,
    notes: d.notes || null,
    sync_status: 'synced',
    updated_at: d.updated_at || null,
    created_at: d.created_at || null,
  }));
  try {
    await db.donations.where('user_id').equals(userId).delete();
    await db.donations.bulkPut(donations);
  } catch (e) {
    /* ignore */
  }
  await refreshReceiptsFromServer(donations);
}

export async function refreshReceiptsFromServer(donations = []) {
  const userId = getCurrentUserId();
  if (!userId) return;
  try {
    const { res, data } = await apiJson('/api/receipts');
    if (!res.ok || !data || !data.receipts) return;
    const receipts = data.receipts.map((r) => ({
      id: r.id,
      key: r.key,
      file_name: r.file_name || null,
      content_type: r.content_type || null,
      size: r.size || null,
      donation_id: r.donation_id,
      uploaded_at: r.created_at || new Date().toISOString(),
    }));
    try {
      await db.transaction('rw', db.receipts, async () => {
        if (donations.length > 0) {
          await db.receipts
            .where('donation_id')
            .anyOf(donations.map((d) => d.id))
            .delete();
        } else {
          await db.receipts.clear();
        }
        await db.receipts.bulkPut(receipts);
      });
    } catch (e) {
      /* ignore */
    }
  } catch (e) {
    console.error('Failed to refresh receipts', e);
  }
}

const CHARITY_CACHE_TTL_MS = 1000 * 60 * 60 * 24 * 30; // 30 days

export async function refreshCharitiesCache() {
  const userId = getCurrentUserId();
  if (!userId) return [];
  const list = await fetchCharitiesFromServer();
  const now = Date.now();
  const cached = list.map((c) => ({
    id: c.id,
    user_id: userId,
    name: c.name,
    ein: c.ein || '',
    category: c.category || null,
    status: c.status || null,
    classification: c.classification || null,
    nonprofit_type: c.nonprofit_type || null,
    deductibility: c.deductibility || null,
    street: c.street || null,
    city: c.city || null,
    state: c.state || null,
    zip: c.zip || null,
    cached_at: now,
  }));
  try {
    await db.charities.where('user_id').equals(userId).delete();
    await db.charities.bulkPut(cached);
  } catch (e) {
    /* ignore */
  }
  return cached;
}

export function isCharityCacheFresh(entry) {
  if (!entry || !entry.cached_at) return false;
  return Date.now() - entry.cached_at <= CHARITY_CACHE_TTL_MS;
}
