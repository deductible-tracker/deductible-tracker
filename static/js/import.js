import db from './db.js';

export async function importCSV(csvString) {
    const res = await fetch('/api/donations/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ csv: csvString })
    });
    if (!res.ok) throw new Error('Import failed');
    return await res.json();
}

// Basic file picker for CSV import
export function createCsvImportInput() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.csv,text/csv';
    input.style.display = 'none';
    input.addEventListener('change', async (e) => {
        const f = e.target.files && e.target.files[0];
        if (!f) return;
        try {
            const txt = await f.text();
            const result = await importCSV(txt);
            alert('Imported ' + (result.imported || 0) + ' rows');
        } catch (err) {
            console.error(err);
            alert('Import failed');
        } finally {
            document.body.removeChild(input);
        }
    });
    document.body.appendChild(input);
    input.click();
}
