import { apiJson } from './services/http.js';

export async function suggestValuations(query) {
    const { res, data } = await apiJson('/api/valuations/suggest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query })
    });
    if (!res.ok) throw new Error('Valuation suggest failed');
    return data;
}

// Expose for quick console usage
window.suggestValuations = suggestValuations;

// Example helper to prompt and show suggestions
export function promptSuggest() {
    const q = prompt('Enter item name to get valuation suggestions:');
    if (!q) return;
    suggestValuations(q).then(r => {
        console.log('Suggestions:', r);
        alert('Suggestions logged to console');
    }).catch(e => {
        console.error(e);
        alert('Suggestion request failed');
    });
}

window.promptSuggestValuations = promptSuggest;
