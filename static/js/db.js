// static/js/db.js
// Dexie instance
const db = new Dexie('DeductibleTrackerDB');

// Bump DB version after schema changes so Dexie doesn't warn about runtime schema extension.
db.version(6).stores({
    donations: 'id, user_id, year, date, charity_id, [user_id+sync_status+date], [user_id+charity_id]',
    items: 'id, donation_id',
    charities: 'id, user_id, ein, name, cached_at, [user_id+ein], [user_id+name]',
    val_categories: 'id, parent_id',
    val_items: 'id, category_id, name', // Valuation DB
    receipts: 'id, donation_id, key, uploaded_at, [donation_id+uploaded_at]',
    sync_queue: '++id, user_id, table, item_id, action, timestamp'
}).upgrade(async tx => {
    // Clear user-scoped caches on schema change to avoid cross-user leakage
    await Promise.all([
        tx.table('donations').clear(),
        tx.table('receipts').clear(),
        tx.table('charities').clear(),
        tx.table('sync_queue').clear()
    ]);
});

export default db;
