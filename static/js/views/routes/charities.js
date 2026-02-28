export async function renderCharitiesRoute(deps) {
    const { db, deleteCharityOnServer, escapeHtml, getCurrentUserId, navigate, refreshCharitiesCache } = deps;
    const root = document.getElementById('route-content') || document.getElementById('app');
    const userId = getCurrentUserId();
    let charities = [];
    try {
        if (userId) { charities = await refreshCharitiesCache(); }
    } catch {
        charities = userId ? await db.charities.where('user_id').equals(userId).toArray() : [];
    }

    const formatAddress = (c) => {
        const parts = [c.street, c.city, c.state, c.zip].map(v => (v || '').trim()).filter(Boolean);
        return parts.length ? parts.join(', ') : '—';
    };

    root.innerHTML = `
        <div class="mx-auto max-w-7xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Charities</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Manage your nonprofit directory and keep compliance details handy.</p>
                </div>
                <button id="btn-new-charity" class="dt-btn-primary">New Charity</button>
            </div>
            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-700">
                        <thead class="bg-slate-50 dark:bg-slate-700/50">
                            <tr>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Name</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">EIN</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Category</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Status</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Deductibility</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Address</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Actions</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-700 bg-white dark:bg-slate-800">
                            ${charities.length === 0 ? '<tr><td colspan="7" class="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">No cached charities.</td></tr>' : charities.map(c => `
                                <tr class="hover:bg-slate-50 dark:bg-slate-700/50/70">
                                    <td class="px-5 py-3 text-sm font-medium text-slate-900 dark:text-slate-100">${escapeHtml(c.name)}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700 dark:text-slate-300">${escapeHtml(c.ein || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700 dark:text-slate-300">${escapeHtml(c.category || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700 dark:text-slate-300">${escapeHtml(c.status || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700 dark:text-slate-300">${escapeHtml(c.deductibility || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700 dark:text-slate-300">${escapeHtml(formatAddress(c))}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600 dark:text-slate-300">
                                        <button class="edit-charity-btn dt-btn-secondary px-3 py-1.5" data-id="${c.id}">Edit</button>
                                        <button class="delete-charity-btn dt-btn-danger ml-2 px-3 py-1.5" data-id="${c.id}">Delete</button>
                                    </td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
                <div class="space-y-3 p-4 md:hidden">
                    ${charities.length === 0 ? '<div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 text-sm text-slate-500 dark:text-slate-400">No cached charities.</div>' : charities.map(c => `
                        <article class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4">
                            <p class="text-sm font-semibold text-slate-900 dark:text-slate-100">${escapeHtml(c.name)}</p>
                            <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">${escapeHtml(c.ein || 'No EIN')}</p>
                            <p class="mt-2 text-sm text-slate-600 dark:text-slate-300">${escapeHtml(formatAddress(c))}</p>
                            <div class="mt-3 flex gap-2">
                                <button class="edit-charity-btn dt-btn-secondary flex-1 px-3 py-1.5" data-id="${c.id}">Edit</button>
                                <button class="delete-charity-btn dt-btn-danger flex-1 px-3 py-1.5" data-id="${c.id}">Delete</button>
                            </div>
                        </article>
                    `).join('')}
                </div>
            </div>
        </div>
    `;

    document.getElementById('btn-new-charity')?.addEventListener('click', () => navigate('/charities/new'));

    document.querySelectorAll('.edit-charity-btn').forEach(button => {
        button.addEventListener('click', (e) => {
            navigate(`/charities/edit/${encodeURIComponent(e.currentTarget.dataset.id)}`);
        });
    });

    document.querySelectorAll('.delete-charity-btn').forEach(b => {
        b.addEventListener('click', async (e) => {
            const charityId = e.currentTarget.dataset.id;
            if (!confirm('Delete cached charity?')) return;
            try {
                const uid = getCurrentUserId();
                if (uid && charityId) {
                    await deleteCharityOnServer(charityId);
                    await db.charities.delete(charityId);
                }
                await renderCharitiesRoute(deps);
            } catch (err) {
                console.error(err);
                alert(err.message || 'Failed to delete');
            }
        });
    });
}

function buildCharityFormHtmlRoute({ title, isEditMode, existing }, deps) {
    const { escapeHtml } = deps;
    const e = existing || {};
    return `
        <div class="mx-auto max-w-3xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">${title}</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">${isEditMode ? 'Update the details for this charity.' : 'Add a new charity to your directory.'}</p>
                </div>
                <button id="btn-back-charities" class="dt-btn-secondary">Back</button>
            </div>
            <div class="dt-panel p-5 sm:p-6">
                <form id="charity-page-form" class="space-y-4">
                    <div>
                        <label class="dt-label">Name</label>
                        <input id="charity-name" required class="dt-input" value="${escapeHtml(e.name || '')}" autocomplete="off" />
                        <div id="charity-name-suggestions" class="mt-1 hidden max-h-48 overflow-auto rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800"></div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">EIN</label>
                            <input id="charity-ein" class="dt-input" value="${escapeHtml(e.ein || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Type of Nonprofit</label>
                            <input id="charity-nonprofit-type" class="dt-input" value="${escapeHtml(e.nonprofit_type || '')}" placeholder="e.g. 501(c)(3)" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Category</label>
                            <input id="charity-category" class="dt-input" value="${escapeHtml(e.category || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Status</label>
                            <input id="charity-status" class="dt-input" value="${escapeHtml(e.status || '')}" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Classification</label>
                            <input id="charity-classification" class="dt-input" value="${escapeHtml(e.classification || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Deductibility</label>
                            <input id="charity-deductibility" class="dt-input" value="${escapeHtml(e.deductibility || '')}" />
                        </div>
                    </div>
                    <div>
                        <label class="dt-label">Street Address</label>
                        <input id="charity-street" class="dt-input" value="${escapeHtml(e.street || '')}" />
                    </div>
                    <div class="grid gap-4 sm:grid-cols-3">
                        <div>
                            <label class="dt-label">City</label>
                            <input id="charity-city" ${!isEditMode ? 'required' : ''} class="dt-input" value="${escapeHtml(e.city || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">State</label>
                            <input id="charity-state" ${!isEditMode ? 'required' : ''} class="dt-input" value="${escapeHtml(e.state || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Zip Code</label>
                            <input id="charity-zip" class="dt-input" value="${escapeHtml(e.zip || '')}" />
                        </div>
                    </div>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">${isEditMode ? 'Save Changes' : 'Add Charity'}</button>
                    </div>
                </form>
            </div>
        </div>
    `;
}

async function bindCharityFormHandlersRoute({ userId, existingCharity, isEditMode }, deps) {
    const {
        apiJson,
        createOrGetCharityOnServer,
        db,
        escapeHtml,
        lookupCharityByEinOnServer,
        navigate,
        updateCharityOnServer,
        updateTotals,
    } = deps;

    const norm = (value) => {
        if (value === null || value === undefined) return null;
        const trimmed = String(value).trim();
        return trimmed ? trimmed : null;
    };

    document.getElementById('btn-back-charities')?.addEventListener('click', () => navigate('/charities'));

    const nameInput = document.getElementById('charity-name');
    const einInput = document.getElementById('charity-ein');
    const nonprofitTypeInput = document.getElementById('charity-nonprofit-type');
    const categoryInput = document.getElementById('charity-category');
    const statusInput = document.getElementById('charity-status');
    const classificationInput = document.getElementById('charity-classification');
    const deductibilityInput = document.getElementById('charity-deductibility');
    const streetInput = document.getElementById('charity-street');
    const cityInput = document.getElementById('charity-city');
    const stateInput = document.getElementById('charity-state');
    const zipInput = document.getElementById('charity-zip');
    const suggestions = document.getElementById('charity-name-suggestions');

    if (!isEditMode && nameInput) {
        let searchTimer = null;
        nameInput.addEventListener('input', () => {
            const query = nameInput.value.trim();
            if (searchTimer) clearTimeout(searchTimer);
            if (!query || query.length < 2) { suggestions.innerHTML = ''; suggestions.classList.add('hidden'); return; }
            searchTimer = setTimeout(async () => {
                try {
                    const { res, data } = await apiJson(`/api/charities/search?q=${encodeURIComponent(query)}`);
                    if (!res.ok || !data || !Array.isArray(data.results) || data.results.length === 0) {
                        suggestions.innerHTML = ''; suggestions.classList.add('hidden'); return;
                    }
                    suggestions.innerHTML = data.results.slice(0, 7).map(item => `
                        <button type="button" class="charity-suggestion-item w-full border-b border-slate-100 dark:border-slate-700 p-2 text-left last:border-b-0 hover:bg-slate-50 dark:bg-slate-700/50" data-name="${escapeHtml(item.name || '')}" data-ein="${escapeHtml(item.ein || '')}" data-location="${escapeHtml(item.location || '')}">
                            <div class="text-sm font-medium text-slate-800">${escapeHtml(item.name || '')}</div>
                            <div class="text-xs text-slate-500 dark:text-slate-400">${escapeHtml(item.ein || '')}${item.location ? ` • ${escapeHtml(item.location)}` : ''}</div>
                        </button>
                    `).join('');
                    suggestions.classList.remove('hidden');

                    suggestions.querySelectorAll('.charity-suggestion-item').forEach(button => {
                        button.addEventListener('click', async () => {
                            const selectedName = button.dataset.name || '';
                            const selectedEin = button.dataset.ein || '';
                            const selectedLocation = button.dataset.location || '';
                            nameInput.value = selectedName;
                            einInput.value = selectedEin;
                            if (selectedLocation && selectedLocation.includes(',')) {
                                const [cityPart, statePart] = selectedLocation.split(',');
                                if (cityPart && !cityInput.value.trim()) cityInput.value = cityPart.trim();
                                if (statePart && !stateInput.value.trim()) stateInput.value = statePart.trim();
                            }
                            suggestions.classList.add('hidden');
                            suggestions.innerHTML = '';
                            if (!selectedEin) return;
                            try {
                                const detail = await lookupCharityByEinOnServer(selectedEin);
                                if (!detail) return;
                                if (detail.name) nameInput.value = detail.name;
                                if (detail.ein) einInput.value = detail.ein;
                                if (detail.nonprofit_type) nonprofitTypeInput.value = detail.nonprofit_type;
                                if (detail.category) categoryInput.value = detail.category;
                                if (detail.status) statusInput.value = detail.status;
                                if (detail.classification) classificationInput.value = detail.classification;
                                if (detail.deductibility) deductibilityInput.value = detail.deductibility;
                                if (detail.street) streetInput.value = detail.street;
                                if (detail.city) cityInput.value = detail.city;
                                if (detail.state) stateInput.value = detail.state;
                                if (detail.zip) zipInput.value = detail.zip;
                            } catch (detailErr) { console.warn('Charity EIN lookup failed', detailErr); }
                        });
                    });
                } catch (err) { console.error('Charity typeahead failed', err); suggestions.classList.add('hidden'); }
            }, 300);
        });
    }

    document.getElementById('charity-page-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const name = nameInput.value.trim();
        const ein = einInput.value.trim() || '';
        const category = norm(categoryInput.value);
        const status = norm(statusInput.value);
        const classification = norm(classificationInput.value);
        const nonprofit_type = norm(nonprofitTypeInput.value);
        const deductibility = norm(deductibilityInput.value);
        const street = norm(streetInput.value);
        const city = norm(cityInput.value);
        const state = norm(stateInput.value);
        const zip = norm(zipInput.value);
        if (!name) { alert('Name required'); return; }
        if (!isEditMode && (!city || !state)) { alert('City and State are required'); return; }

        try {
            let charity = null;
            if (isEditMode && existingCharity?.id) {
                const updatePayload = { name, ein: ein || null, category, status, classification, nonprofit_type, deductibility, street, city, state, zip };
                const resp = await updateCharityOnServer(existingCharity.id, updatePayload);
                charity = resp && resp.charity ? resp.charity : { id: existingCharity.id, ...updatePayload };
            } else {
                const resp = await createOrGetCharityOnServer({ name, ein: ein || null, category, status, classification, nonprofit_type, deductibility, street, city, state, zip });
                charity = resp && resp.charity ? resp.charity : null;
                if (!charity || !charity.id) throw new Error('Failed to create charity');
            }

            if (charity && userId) {
                await db.charities.put({
                    id: charity.id, user_id: userId, name: charity.name, ein: charity.ein || '',
                    category: charity.category || null, status: charity.status || null,
                    classification: charity.classification || null, nonprofit_type: charity.nonprofit_type || null,
                    deductibility: charity.deductibility || null, street: charity.street || null,
                    city: charity.city || null, state: charity.state || null,
                    zip: charity.zip || null, cached_at: Date.now()
                });
            }

            await updateTotals();
            await navigate('/charities');
        } catch (err) {
            console.error(err);
            alert(isEditMode ? 'Failed to update charity' : 'Failed to add charity');
        }
    });
}

export async function renderCharityNewRoute(deps) {
    const root = document.getElementById('route-content') || document.getElementById('app');
    const userId = deps.getCurrentUserId();
    root.innerHTML = buildCharityFormHtmlRoute({ title: 'New Charity', isEditMode: false, existing: null }, deps);
    await bindCharityFormHandlersRoute({ userId, existingCharity: null, isEditMode: false }, deps);
}

export async function renderCharityEditRoute(charityId, deps) {
    const root = document.getElementById('route-content') || document.getElementById('app');
    const userId = deps.getCurrentUserId();
    let charities = [];
    try {
        charities = userId ? await deps.db.charities.where('user_id').equals(userId).toArray() : [];
    } catch { /* ignore */ }
    const existing = charities.find(c => c.id === charityId);
    if (!existing) { alert('Charity not found'); await deps.navigate('/charities'); return; }
    root.innerHTML = buildCharityFormHtmlRoute({ title: 'Edit Charity', isEditMode: true, existing }, deps);
    await bindCharityFormHandlersRoute({ userId, existingCharity: existing, isEditMode: true }, deps);
}
