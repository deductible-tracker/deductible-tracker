import db from './db.js';

const API_BASE = '/api';

const getAuthHeaders = () => {
    const token = localStorage.getItem('jwt');
    return token ? { 'Authorization': `Bearer ${token}` } : {};
};

export const Sync = {
    async pushChanges() {
        const queue = await db.sync_queue.toArray();
        if (queue.length === 0) return;

        console.log('Pushing changes...', queue.length);

        for (const task of queue) {
            try {
                let success = false;
                if (task.table === 'donations') {
                    if (task.action === 'create') {
                        const donation = await db.donations.get(task.item_id);
                        if (donation) {
                            const res = await fetch(`${API_BASE}/donations`, {
                                method: 'POST',
                                headers: { 
                                    'Content-Type': 'application/json',
                                    ...getAuthHeaders()
                                },
                                body: JSON.stringify(donation)
                            });
                            if (res.ok) success = true;
                            if (res.status === 401) {
                                console.warn('Unauthorized during sync. Stopping.');
                                return;
                            }
                        }
                    }
                    // Handle update/delete...
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
    },

    async pullChanges() {
        // Mock pull
        console.log('Pulling changes...');
        try {
            const res = await fetch(`${API_BASE}/donations`, {
                headers: getAuthHeaders()
            });
            if (res.ok) {
                const data = await res.json();
                // Merge logic here
                // for (const remoteDonation of data.donations) { ... }
            }
        } catch (e) {
            console.error('Pull failed', e);
        }
    },

    async queueAction(table, item, action) {
        // 1. Apply to local DB immediately (Optimistic UI)
        if (action === 'create' || action === 'update') {
            await db.table(table).put({ ...item, sync_status: 'pending' });
        } else if (action === 'delete') {
            await db.table(table).delete(item.id);
        }

        // 2. Add to Queue
        await db.sync_queue.add({
            table,
            item_id: item.id,
            action,
            timestamp: Date.now()
        });

        // 3. Trigger Sync (fire and forget)
        this.pushChanges();
    }
};
