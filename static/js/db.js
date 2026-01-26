// static/js/db.js
// Dexie instance
const db = new Dexie('DeductibleTrackerDB');

db.version(1).stores({
    donations: 'id, year, date, charity_name, [sync_status+date]', // sync_status for filtering
    items: 'id, donation_id',
    charities: 'ein, name', // Cache
    val_categories: 'id, parent_id',
    val_items: 'id, category_id, name', // Valuation DB
    sync_queue: '++id, table, item_id, action, timestamp' // Outbox pattern
});

export default db;
