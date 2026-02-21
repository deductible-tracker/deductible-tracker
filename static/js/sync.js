import db from './db.js';

const API_BASE = '/api';

function getCurrentUserId() {
    try {
        const raw = localStorage.getItem('current_user');
        if (!raw) return null;
        const parsed = JSON.parse(raw);
        return parsed && parsed.id ? parsed.id : null;
    } catch (e) {
        return null;
    }
}

export const Sync = {
    async pushChanges() {
        const userId = getCurrentUserId();
        if (!userId) return;
        const queue = await db.sync_queue.where('user_id').equals(userId).toArray();
        if (queue.length === 0) return;

        console.log('Pushing changes...', queue.length);

        for (const task of queue) {
            try {
                let success = false;
                if (task.table === 'donations') {
                    if (task.action === 'create') {
                        const donation = await db.donations.get(task.item_id);
                        if (donation) {
                            let charityName = '';
                            if (donation.charity_id) {
                                try {
                                    const charity = await db.charities.get(donation.charity_id);
                                    charityName = charity && charity.name ? charity.name : '';
                                } catch (e) { /* ignore */ }
                            }
                            const payload = {
                                id: donation.id,
                                date: donation.date,
                                charity_id: donation.charity_id || null,
                                charity_name: charityName,
                                category: donation.category || null,
                                amount: donation.amount ?? null,
                                notes: donation.notes || null,
                                updated_at: donation.updated_at || null
                            };
                            const res = await fetch(`${API_BASE}/donations`, {
                                method: 'POST',
                                headers: { 
                                    'Content-Type': 'application/json'
                                },
                                credentials: 'include',
                                body: JSON.stringify(payload)
                            });
                            if (res.ok) {
                                success = true;
                            } else {
                                let bodyText = '';
                                try { bodyText = await res.text(); } catch (e) { /* ignore */ }
                                console.warn('Donation sync failed', res.status, bodyText);
                            }
                            if (res.status === 401) {
                                console.warn('Unauthorized during sync. Stopping.');
                                return;
                            }
                        }
                    }
                    if (task.action === 'update') {
                        const donation = await db.donations.get(task.item_id);
                        if (donation) {
                            const res = await fetch(`${API_BASE}/donations/${task.item_id}`, {
                                method: 'PUT',
                                headers: { 'Content-Type': 'application/json' },
                                credentials: 'include',
                                body: JSON.stringify(donation)
                            });
                            if (res.ok) {
                                success = true;
                            } else {
                                let bodyText = '';
                                try { bodyText = await res.text(); } catch (e) { /* ignore */ }
                                console.warn('Donation update failed', res.status, bodyText);
                            }
                            if (res.status === 401) { console.warn('Unauthorized during sync. Stopping.'); return; }
                        }
                    }
                    if (task.action === 'delete') {
                        // call server delete so it can soft-delete and propagate
                        const res = await fetch(`${API_BASE}/donations/${task.item_id}`, {
                            method: 'DELETE',
                            credentials: 'include'
                        });
                        if (res.ok) { success = true; }
                        else {
                            let bodyText = '';
                            try { bodyText = await res.text(); } catch (e) { /* ignore */ }
                            console.warn('Donation delete failed', res.status, bodyText);
                        }
                        if (res.status === 401) { console.warn('Unauthorized during sync. Stopping.'); return; }
                    }
                }

                if (task.table === 'receipts' && task.action === 'create') {
                    const receipt = await db.receipts.get(task.item_id);
                    if (receipt) {
                        if (!receipt.donation_id) {
                            console.warn('Skipping receipt sync without donation_id', receipt.id);
                            continue;
                        }
                        const res = await fetch(`${API_BASE}/receipts/confirm`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            credentials: 'include',
                            body: JSON.stringify({
                                key: receipt.key,
                                file_name: receipt.file_name,
                                content_type: receipt.content_type,
                                size: receipt.size,
                                donation_id: receipt.donation_id
                            })
                        });
                        if (res.ok) {
                            success = true;
                            try {
                                const body = await res.json();
                                if (body && body.id) {
                                    await db.receipts.update(receipt.id, { server_id: body.id });
                                }
                            } catch (e) { /* ignore */ }
                        } else {
                            let bodyText = '';
                            try { bodyText = await res.text(); } catch (e) { /* ignore */ }
                            console.warn('Receipt sync failed', res.status, bodyText);
                        }
                        if (res.status === 401) {
                            console.warn('Unauthorized during sync. Stopping.');
                            return;
                        }
                    }
                }

                if (success) {
                    await db.sync_queue.delete(task.id);
                    // Update sync_status on item if needed
                    if (task.table === 'donations') {
                         await db.donations.update(task.item_id, { sync_status: 'synced' });
                    }
                }
            } catch (err) {
                console.error('Sync failed for task', task, err);
                // Stop processing to maintain order or continue? 
                // Simple: continue, retry later.
            }
        }

        try { window.dispatchEvent(new CustomEvent('sync-queue-changed')); } catch (e) { /* ignore */ }
    },

    async pullChanges() {
        console.log('Pulling changes...');
        const userId = getCurrentUserId();
        if (!userId) return;
        const lastKey = `last_sync_${userId}`;
        const lastSync = localStorage.getItem(lastKey);
        const url = lastSync ? `${API_BASE}/donations?since=${encodeURIComponent(lastSync)}` : `${API_BASE}/donations`;
        try {
            const res = await fetch(url, { credentials: 'include' });
            if (!res.ok) {
                if (res.status === 401) { console.warn('Unauthorized during pull'); return; }
                console.warn('Pull failed', res.status);
                return;
            }
            const data = await res.json();
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
                                created_at: remote.created_at || null
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
            try { window.dispatchEvent(new CustomEvent('sync-queue-changed')); } catch (e) { /* ignore */ }
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
            timestamp: Date.now()
        });

        // 3. Trigger Sync (fire and forget)
        this.pushChanges();
        try { window.dispatchEvent(new CustomEvent('sync-queue-changed')); } catch (e) { /* ignore */ }
    }
};
