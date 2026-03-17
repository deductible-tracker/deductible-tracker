export async function renderDonationsRoute(deps) {
  const {
    calculateTaxEstimates,
    db,
    deleteDonationOnServer,
    escapeHtml,
    formatCurrency,
    getCurrentUser,
    getCurrentUserId,
    navigate,
    updateTotals,
  } = deps;

  const root = document.getElementById('route-content') || document.getElementById('app');
  const userId = getCurrentUserId();
  const charities = userId ? await db.charities.where('user_id').equals(userId).toArray() : [];
  const charityNameMap = new Map(charities.map((c) => [c.id, c.name || 'Unknown charity']));
  const donations = userId
    ? (await db.donations.where('user_id').equals(userId).toArray()).sort((a, b) =>
        String(b.date || '').localeCompare(String(a.date || ''))
      )
    : [];
  const receipts = await db.receipts.toArray();
  const taxEstimates = await calculateTaxEstimates(
    donations,
    charities,
    receipts,
    getCurrentUser() || {}
  );

  root.innerHTML = `
        <div class="mx-auto max-w-7xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Donations</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Add donations, attach receipts immediately, and keep records audit-ready.</p>
                </div>
                <button id="btn-new-donation" class="dt-btn-primary">New Donation</button>
            </div>

            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-700">
                        <thead class="bg-slate-50 dark:bg-slate-700/50">
                            <tr>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Date</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Charity</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Status</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Category</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Amount</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Estimated Tax Savings</th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Actions</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-700 bg-white dark:bg-slate-800">
                            ${
                              donations.length === 0
                                ? `
                                <tr>
                                    <td colspan="7" class="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">No donations yet.</td>
                                </tr>
                            `
                                : donations
                                    .map(
                                      (d) => `
                                <tr class="hover:bg-slate-50 dark:bg-slate-700/50/70">
                                    <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600 dark:text-slate-300">${escapeHtml(d.date)}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-slate-900 dark:text-slate-100">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600 dark:text-slate-300">
                                        <span class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">${escapeHtml(d.sync_status || 'synced')}</span>
                                    </td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600 dark:text-slate-300">${escapeHtml(d.category || '')}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-slate-900 dark:text-slate-100">${d.amount ? formatCurrency(d.amount) : ''}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-emerald-700 dark:text-emerald-300">${formatCurrency(taxEstimates.perDonation.get(d.id) || 0)}</td>
                                    <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600 dark:text-slate-300">
                                        <button class="edit-donation-btn dt-btn-secondary px-3 py-1.5" data-id="${d.id}">Edit</button>
                                        <button class="manage-receipts-btn dt-btn-secondary ml-2 px-3 py-1.5" data-id="${d.id}">Receipts</button>
                                        <button class="delete-donation-btn dt-btn-danger ml-2 px-3 py-1.5" data-id="${d.id}">Delete</button>
                                    </td>
                                </tr>
                            `
                                    )
                                    .join('')
                            }
                        </tbody>
                    </table>
                </div>
                <div class="space-y-3 p-4 md:hidden">
                    ${
                      donations.length === 0
                        ? '<div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 text-sm text-slate-500 dark:text-slate-400">No donations yet.</div>'
                        : donations
                            .map(
                              (d) => `
                        <article class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4">
                            <div class="flex items-start justify-between gap-3">
                                <div>
                                    <p class="text-sm font-semibold text-slate-900 dark:text-slate-100">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</p>
                                    <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">${escapeHtml(d.date || '')} • ${escapeHtml(d.category || '')}</p>
                                </div>
                                <span class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">${escapeHtml(d.sync_status || 'synced')}</span>
                            </div>
                            <div class="mt-3 grid grid-cols-2 gap-2 text-sm">
                                <div class="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
                                    <p class="text-xs text-slate-500 dark:text-slate-400">Amount</p>
                                    <p class="font-semibold text-slate-900 dark:text-slate-100">${d.amount ? formatCurrency(d.amount) : '$0.00'}</p>
                                </div>
                                <div class="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
                                    <p class="text-xs text-slate-500 dark:text-slate-400">Est. savings</p>
                                    <p class="font-semibold text-emerald-700 dark:text-emerald-300">${formatCurrency(taxEstimates.perDonation.get(d.id) || 0)}</p>
                                </div>
                            </div>
                            <div class="mt-3 flex gap-2">
                                <button class="edit-donation-btn dt-btn-secondary flex-1 px-3 py-1.5" data-id="${d.id}">Edit</button>
                                <button class="manage-receipts-btn dt-btn-secondary flex-1 px-3 py-1.5" data-id="${d.id}">Receipts</button>
                                <button class="delete-donation-btn dt-btn-danger flex-1 px-3 py-1.5" data-id="${d.id}">Delete</button>
                            </div>
                        </article>
                    `
                            )
                            .join('')
                    }
                </div>
            </div>
        </div>
    `;

  document
    .getElementById('btn-new-donation')
    ?.addEventListener('click', () => navigate('/donations/new'));

  document.querySelectorAll('.edit-donation-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      navigate(`/donations/edit/${encodeURIComponent(e.currentTarget.dataset.id)}`);
    });
  });

  document.querySelectorAll('.manage-receipts-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      const id = e.currentTarget.dataset.id;
      openReceiptManagerRoute(id, deps);
    });
  });

  document.querySelectorAll('.delete-donation-btn').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      const id = e.currentTarget.dataset.id;
      if (!confirm('Delete this donation? Associated receipts will also be removed.')) return;
      try {
        await deleteDonationOnServer(id);
        const localReceipts = await db.receipts.where('donation_id').equals(id).toArray();
        for (const r of localReceipts) {
          await db.receipts.delete(r.id);
        }
        await db.donations.delete(id);
        await renderDonationsRoute(deps);
        await updateTotals();
      } catch (err) {
        console.error('Delete failed', err);
        alert('Failed to delete donation');
      }
    });
  });
}

export async function openReceiptManagerRoute(donationId, deps) {
  await deps.navigate(`/donations/receipts/${donationId}`);
}

async function getReceiptDownloadUrlRoute(key, deps) {
  const { getCookie } = deps;
  const res = await fetch('/api/receipts/presign', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRF-Token': getCookie('auth_token'),
    },
    credentials: 'include',
    body: JSON.stringify({ key }),
  });
  if (!res.ok) throw new Error('Presign failed');
  const data = await res.json();
  return data.download_url;
}

function buildDonationFormHtmlRoute(
  { title, desc, submitLabel, categoryPrefill = 'money', existing = null },
  deps
) {
  const { escapeHtml } = deps;
  const d = existing || {};
  let displayNotes = d.notes || '';
  if (d.category === 'items' && displayNotes.startsWith('Item: ')) {
    displayNotes = displayNotes.replace(/^Item: (.*?)(?:\n|$)/, '').trim();
  }
  const currentCategory = d.category || categoryPrefill;

  return `
        <div class="mx-auto max-w-3xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">${title}</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">${desc}</p>
                </div>
                <button id="btn-back-donations" class="dt-btn-secondary">Back</button>
            </div>
            <div class="dt-panel p-5 sm:p-6">
                <form id="donation-page-form" class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Donation Date</label>
                            <input id="donation-date" type="date" required class="dt-input" value="${escapeHtml(d.date || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Category</label>
                            <select id="donation-category" class="dt-input">
                                <option value="items" ${currentCategory === 'items' ? 'selected' : ''}>Items</option>
                                <option value="money" ${currentCategory === 'money' ? 'selected' : ''}>Money</option>
                                <option value="mileage" ${currentCategory === 'mileage' ? 'selected' : ''}>Mileage</option>
                            </select>
                        </div>
                    </div>

                    <div id="valuation-fields" class="${currentCategory === 'items' ? '' : 'hidden'} space-y-4 rounded-xl border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-700/50 p-4">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div>
                                <label class="dt-label">Item Type</label>
                                <input id="valuation-item-input" type="text" placeholder="Search item type" class="dt-input" autocomplete="off" />
                                <div id="valuation-suggestions" class="mt-1 hidden max-h-48 overflow-auto rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 absolute z-10 w-64 shadow-lg"></div>
                            </div>
                            <div id="brand-new-price-container" class="hidden">
                                <label class="dt-label">Brand New Price ($)</label>
                                <input id="brand-new-price" type="number" step="0.01" class="dt-input" placeholder="Price when new" />
                            </div>
                        </div>
                        <p id="valuation-hint" class="text-xs text-slate-500 dark:text-slate-400 italic">Select an item to see suggested valuation ranges.</p>
                    </div>

                    <div>
                        <label class="dt-label">Charity</label>
                        <input id="donation-charity-input" type="text" required placeholder="Search or type to add" class="dt-input" autocomplete="off" value="${escapeHtml(d._charityName || '')}" />
                        <input id="donation-charity-id" type="hidden" value="${escapeHtml(d.charity_id || '')}" />
                        <input id="donation-charity-ein" type="hidden" />
                        <div id="charity-suggestions" class="mt-1 hidden max-h-48 overflow-auto rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800"></div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Amount ($)</label>
                            <input id="donation-amount" type="number" step="0.01" class="dt-input" value="${d.amount != null ? escapeHtml(String(d.amount)) : ''}" />
                        </div>
                        <div>
                            <label class="dt-label">Receipts</label>
                            <input id="donation-receipts" type="file" multiple accept="image/*,application/pdf" class="dt-input" />
                        </div>
                    </div>
                    <div>
                        <label class="dt-label">Notes</label>
                        <textarea id="donation-notes" rows="3" class="dt-input">${escapeHtml(displayNotes || '')}</textarea>
                    </div>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">${submitLabel}</button>
                    </div>
                </form>
            </div>
        </div>
    `;
}

async function bindDonationFormHandlersRoute({ userId, charities, existingDonation }, deps) {
  const {
    createDonationOnServer,
    createOrGetCharityOnServer,
    db,
    escapeHtml,
    isCharityCacheFresh,
    navigate,
    Sync,
    updateDonationOnServer,
    updateTotals,
  } = deps;

  const isEditMode = !!existingDonation;
  const charityInput = document.getElementById('donation-charity-input');
  const charityIdInput = document.getElementById('donation-charity-id');
  const charityEinInput = document.getElementById('donation-charity-ein');
  const suggestionsBox = document.getElementById('charity-suggestions');

  const categorySelect = document.getElementById('donation-category');
  const valuationFields = document.getElementById('valuation-fields');
  const valuationInput = document.getElementById('valuation-item-input');
  const valuationSuggestions = document.getElementById('valuation-suggestions');
  const brandNewPriceContainer = document.getElementById('brand-new-price-container');
  const brandNewPriceInput = document.getElementById('brand-new-price');
  const amountInput = document.getElementById('donation-amount');
  const valuationHint = document.getElementById('valuation-hint');

  if (isEditMode && existingDonation.category === 'items' && existingDonation.notes) {
    const match = existingDonation.notes.match(/^Item: (.*?)(?:\n|$)/);
    if (match) {
      valuationInput.value = match[1];
    }
  }

  categorySelect?.addEventListener('change', () => {
    if (categorySelect.value === 'items') {
      valuationFields?.classList.remove('hidden');
    } else {
      valuationFields?.classList.add('hidden');
    }
  });

  let valTimer = null;
  valuationInput?.addEventListener('input', () => {
    const q = valuationInput.value.trim();
    if (valTimer) clearTimeout(valTimer);
    if (!q) {
      valuationSuggestions.innerHTML = '';
      valuationSuggestions.classList.add('hidden');
      return;
    }

    valTimer = setTimeout(async () => {
      try {
        const { getCookie } = deps;
        const res = await fetch('/api/valuations/suggest', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCookie('auth_token'),
          },
          credentials: 'include',
          body: JSON.stringify({ query: q }),
        });
        if (res.ok) {
          const data = await res.json();
          const suggestions = data.suggestions || [];
          if (suggestions.length === 0) {
            valuationSuggestions.innerHTML =
              '<div class="p-2 text-sm text-slate-500">No items found.</div>';
          } else {
            valuationSuggestions.innerHTML = suggestions
              .map(
                (s) => `
              <div class="cursor-pointer p-2 hover:bg-slate-50 dark:hover:bg-slate-700" data-name="${escapeHtml(s.name)}" data-min="${s.min}" data-max="${s.max}">
                <div class="font-medium text-slate-900 dark:text-slate-100">${escapeHtml(s.name)}</div>
                <div class="text-xs text-slate-500 dark:text-slate-400">Suggested: ($${s.min}-$${s.max})</div>
              </div>
            `
              )
              .join('');
          }
          valuationSuggestions.classList.remove('hidden');

          valuationSuggestions.querySelectorAll('div[data-name]').forEach((el) => {
            el.addEventListener('click', () => {
              const name = el.dataset.name;
              const min = el.dataset.min;
              const max = el.dataset.max;
              valuationInput.value = name;
              valuationSuggestions.classList.add('hidden');

              if (
                name.toLowerCase().includes('other') ||
                name.toLowerCase().includes('not listed')
              ) {
                brandNewPriceContainer?.classList.remove('hidden');
                valuationHint.textContent =
                  'Valuation will be calculated at 30% of the brand new price.';
              } else {
                brandNewPriceContainer?.classList.add('hidden');
                valuationHint.textContent = `Suggested value for "${name}": $${min || 0} - $${max || 0}. Please enter an amount within or near this range.`;
                if (min) {
                  amountInput.value = min;
                }
              }
            });
          });
        }
      } catch (err) {
        console.error('Valuation search failed', err);
      }
    }, 300);
  });

  // Close suggestions on click outside
  document.addEventListener('click', (e) => {
    if (!valuationInput?.contains(e.target) && !valuationSuggestions?.contains(e.target)) {
      valuationSuggestions?.classList.add('hidden');
    }
  });

  brandNewPriceInput?.addEventListener('input', () => {
    const price = parseFloat(brandNewPriceInput.value) || 0;
    if (price > 0) {
      amountInput.value = (price * 0.3).toFixed(2);
    }
  });

  document
    .getElementById('btn-back-donations')
    ?.addEventListener('click', () => navigate('/donations'));

  const dateInput = document.getElementById('donation-date');
  if (dateInput && !dateInput.value) {
    dateInput.value = new Date().toISOString().split('T')[0];
  }

  if (isEditMode && charityInput && !charityInput.value && existingDonation.charity_id) {
    const c = charities.find((ch) => ch.id === existingDonation.charity_id);
    if (c) charityInput.value = c.name;
  }

  let suggestionTimer = null;
  charityInput?.addEventListener('input', (e) => {
    charityIdInput.value = '';
    charityEinInput.value = '';
    const q = e.target.value.trim();
    if (suggestionTimer) clearTimeout(suggestionTimer);
    if (!q) {
      suggestionsBox.innerHTML = '';
      suggestionsBox.classList.add('hidden');
      return;
    }

    suggestionTimer = setTimeout(async () => {
      try {
        const qLower = q.toLowerCase();
        let localMatches = [];
        try {
          localMatches = await db.charities
            .where('user_id')
            .equals(userId)
            .filter((c) => isCharityCacheFresh(c) && (c.name || '').toLowerCase().includes(qLower))
            .toArray();
        } catch (le) {
          console.warn('Local charity lookup failed', le);
        }

        let remote = [];
        try {
          const res = await fetch(`/api/charities/search?q=${encodeURIComponent(q)}`, {
            credentials: 'include',
          });
          if (res.ok) {
            const data = await res.json();
            remote = data.results || [];
          }
        } catch (re) {
          console.warn('Remote charity search failed', re);
        }

        const seen = new Set();
        const merged = [];
        for (const c of localMatches) {
          const key = ((c.ein || '').trim() || (c.name || '').trim()).toLowerCase();
          if (!seen.has(key)) {
            seen.add(key);
            merged.push({
              id: c.id,
              ein: c.ein || '',
              name: c.name,
              location: '',
              source: 'local',
            });
          }
        }
        for (const r of remote) {
          const key = ((r.ein || '').trim() || (r.name || '').trim()).toLowerCase();
          if (!seen.has(key)) {
            seen.add(key);
            merged.push({
              id: '',
              ein: r.ein || '',
              name: r.name,
              location: r.location || '',
              source: 'remote',
            });
          }
        }

        if (merged.length === 0) {
          suggestionsBox.innerHTML =
            '<div class="p-2 text-sm text-slate-500 dark:text-slate-400">No matches</div>';
          suggestionsBox.classList.remove('hidden');
          return;
        }

        suggestionsBox.innerHTML = merged
          .map(
            (r) => `
                    <div class="flex cursor-pointer items-center justify-between p-2 hover:bg-slate-50 dark:bg-slate-700/50" data-id="${escapeHtml(r.id || '')}" data-ein="${escapeHtml(r.ein)}" data-name="${escapeHtml(r.name)}">
                        <div>
                            <div class="font-medium text-slate-900 dark:text-slate-100">${escapeHtml(r.name)}</div>
                            <div class="text-xs text-slate-400">${escapeHtml(r.location || (r.source === 'local' ? 'Cached' : ''))}</div>
                        </div>
                        <div class="ml-4 text-xs text-slate-500 dark:text-slate-400">${r.source === 'local' ? 'Saved' : ''}</div>
                    </div>
                `
          )
          .join('');
        suggestionsBox.classList.remove('hidden');

        suggestionsBox.querySelectorAll('div[data-id][data-ein][data-name]').forEach((el) => {
          el.addEventListener('click', () => {
            charityInput.value = el.dataset.name || '';
            charityIdInput.value = el.dataset.id || '';
            charityEinInput.value = el.dataset.ein || '';
            suggestionsBox.classList.add('hidden');
          });
        });
      } catch (err) {
        console.error('Charity search failed', err);
        suggestionsBox.classList.add('hidden');
      }
    }, 300);
  });

  document.getElementById('donation-page-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    if (!userId) {
      alert('Please sign in again');
      return;
    }

    const date = document.getElementById('donation-date').value;
    const charity_name = charityInput.value.trim();
    const charity_id = charityIdInput.value.trim();
    const charity_ein = charityEinInput.value.trim();
    let notes = document.getElementById('donation-notes').value.trim();
    const category = document.getElementById('donation-category').value;
    const itemType = valuationInput ? valuationInput.value.trim() : '';

    if (category === 'items' && itemType) {
      const prefix = `Item: ${itemType}`;
      if (!notes.startsWith(prefix)) {
        notes = prefix + (notes ? `\n${notes}` : '');
      }
    }

    const amount = parseFloat(document.getElementById('donation-amount').value) || 0;
    const receiptFiles = Array.from(document.getElementById('donation-receipts').files || []);

    if (!date || !charity_name) {
      alert('Please provide date and charity name');
      return;
    }

    const year = new Date(date).getFullYear();
    try {
      let charityId = charity_id || (isEditMode ? existingDonation.charity_id : '') || '';
      if (!charityId) {
        const resp = await createOrGetCharityOnServer(charity_name, charity_ein || null);
        const charity = resp && resp.charity ? resp.charity : null;
        if (charity && charity.id) {
          charityId = charity.id;
          await db.charities.put({
            id: charity.id,
            user_id: userId,
            name: charity.name,
            ein: charity.ein || '',
            category: charity.category || null,
            status: charity.status || null,
            classification: charity.classification || null,
            nonprofit_type: charity.nonprofit_type || null,
            deductibility: charity.deductibility || null,
            street: charity.street || null,
            city: charity.city || null,
            state: charity.state || null,
            zip: charity.zip || null,
            cached_at: Date.now(),
          });
        }
      }

      const payload = {
        date,
        charity_name,
        charity_id: charityId || null,
        charity_ein: charity_ein || null,
        category,
        amount,
        notes: notes || null,
      };

      let donation;
      const fallbackId = isEditMode ? existingDonation.id : crypto.randomUUID();
      if (isEditMode) {
        await updateDonationOnServer(existingDonation.id, payload);
        donation = {
          ...existingDonation,
          date,
          charity_id: charityId || existingDonation.charity_id,
          notes: notes || null,
          category,
          amount,
          sync_status: 'synced',
        };
        await db.donations.put(donation);
      } else {
        try {
          const res = await createDonationOnServer(payload);
          const serverId = res && res.id ? res.id : fallbackId;
          donation = {
            id: serverId,
            user_id: userId,
            year,
            date,
            charity_id: charityId || null,
            notes: notes || null,
            category,
            amount,
            sync_status: 'synced',
          };
          await db.donations.put(donation);
        } catch {
          donation = {
            id: fallbackId,
            user_id: userId,
            year,
            date,
            charity_id: charityId || null,
            notes: notes || null,
            category,
            amount,
            sync_status: 'pending',
          };
          await Sync.queueAction('donations', donation, 'create');
        }
      }

      if (receiptFiles.length > 0 && donation.sync_status === 'synced') {
        for (const file of receiptFiles) {
          await uploadReceiptForDonationRoute(file, donation.id, deps);
        }
      } else if (receiptFiles.length > 0) {
        alert('Donation saved offline. Upload receipts after sync completes.');
      }

      await updateTotals();
      await navigate('/donations');
    } catch (err) {
      console.error('Failed to save donation', err);
      alert('Failed to save donation');
    }
  });
}

export async function renderDonationNewRoute(deps) {
  const root = document.getElementById('route-content') || document.getElementById('app');
  const userId = deps.getCurrentUserId();
  const charities = userId ? await deps.db.charities.where('user_id').equals(userId).toArray() : [];

  root.innerHTML = buildDonationFormHtmlRoute(
    {
      title: 'New Donation',
      desc: 'Add a donation and optionally attach receipts.',
      submitLabel: 'Save Donation',
      categoryPrefill: 'money',
    },
    deps
  );
  await bindDonationFormHandlersRoute({ userId, charities, existingDonation: null }, deps);
}

export async function renderDonationEditRoute(donationId, deps) {
  const userId = deps.getCurrentUserId();
  const charities = userId ? await deps.db.charities.where('user_id').equals(userId).toArray() : [];
  const existing = await deps.db.donations.get(donationId);
  if (!existing) {
    alert('Donation not found');
    await deps.navigate('/donations');
    return;
  }
  const charityName = charities.find((c) => c.id === existing.charity_id)?.name || '';
  const root = document.getElementById('route-content') || document.getElementById('app');
  root.innerHTML = buildDonationFormHtmlRoute(
    {
      title: 'Edit Donation',
      desc: 'Update the details for this donation.',
      submitLabel: 'Save Changes',
      existing: { ...existing, _charityName: charityName },
    },
    deps
  );
  await bindDonationFormHandlersRoute({ userId, charities, existingDonation: existing }, deps);
}

export async function renderReceiptPageRoute(donationId, deps) {
  const root = document.getElementById('route-content') || document.getElementById('app');
  root.innerHTML = `
        <div class="mx-auto max-w-3xl space-y-5">
            <div class="flex items-center justify-between gap-2">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Manage Receipts</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Upload and preview receipts for this donation.</p>
                </div>
                <button id="btn-back-donations" class="dt-btn-secondary">Back to Donations</button>
            </div>
            <div class="dt-panel p-5 sm:p-6">
                <div class="mb-4">
                    <label class="dt-label">Upload Receipt</label>
                    <input id="receipt-upload-input" type="file" multiple accept="image/*,application/pdf" class="dt-input" />
                    <button id="btn-upload-receipts" class="dt-btn-primary mt-2">Upload</button>
                </div>
                <h4 class="text-sm font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400 mb-2">Attached Receipts</h4>
                <div id="attached-list"></div>
            </div>
        </div>
    `;

  document
    .getElementById('btn-back-donations')
    ?.addEventListener('click', () => deps.navigate('/donations'));

  document.getElementById('btn-upload-receipts')?.addEventListener('click', async () => {
    const fileInput = document.getElementById('receipt-upload-input');
    const files = fileInput ? Array.from(fileInput.files || []) : [];
    if (files.length === 0) {
      alert('Please select a file to upload.');
      return;
    }
    try {
      for (const file of files) {
        await uploadReceiptForDonationRoute(file, donationId, deps);
      }
      fileInput.value = '';
      await refreshAttachedListRoute(donationId, root, deps);
    } catch (err) {
      console.error('Upload failed', err);
      alert('Failed to upload receipt');
    }
  });

  await refreshAttachedListRoute(donationId, root, deps);
}

async function refreshAttachedListRoute(donationId, container, deps) {
  const normalizedId = donationId && String(donationId).trim() ? String(donationId) : null;
  if (!normalizedId) return;
  const attached = await deps.db.receipts.where('donation_id').equals(normalizedId).toArray();
  const attachedList = container.querySelector('#attached-list');
  if (!attachedList) return;

  attachedList.innerHTML =
    attached.length === 0
      ? '<div class="rounded-xl border border-slate-200 dark:border-slate-700 px-3 py-4 text-sm text-slate-500 dark:text-slate-400">No attached receipts.</div>'
      : attached
          .map(
            (r) => `
            <div class="mb-2 flex items-center justify-between rounded-xl border border-slate-200 dark:border-slate-700 p-3">
                <div>
                    <div class="text-sm font-medium text-slate-900 dark:text-slate-100">${deps.escapeHtml(r.file_name || r.key)}</div>
                    <div class="text-xs text-slate-500 dark:text-slate-400">${r.uploaded_at ? new Date(r.uploaded_at).toLocaleString() : ''}</div>
                </div>
                <div class="flex items-center space-x-2">
                    <button class="preview-receipt-btn dt-btn-secondary px-3 py-1.5" data-key="${deps.escapeHtml(r.key)}">Preview</button>
                </div>
            </div>
        `
          )
          .join('');

  attachedList.querySelectorAll('.preview-receipt-btn').forEach((b) => {
    b.addEventListener('click', async (e) => {
      if (!navigator.onLine) {
        alert('Receipt preview requires an internet connection.');
        return;
      }
      const key = e.currentTarget.dataset.key;
      try {
        const downloadUrl = await getReceiptDownloadUrlRoute(key, deps);
        window.open(downloadUrl, '_blank');
      } catch {
        alert('Preview failed');
      }
    });
  });
}

async function uploadReceiptForDonationRoute(file, donationId, deps) {
  const { getCookie } = deps;
  const uploadRes = await fetch('/api/receipts/upload', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRF-Token': getCookie('auth_token'),
    },
    credentials: 'include',
    body: JSON.stringify({ file_type: file.type }),
  });
  if (!uploadRes.ok) throw new Error('Failed to request upload URL');

  const uploadData = await uploadRes.json();
  const putRes = await fetch(uploadData.upload_url, {
    method: 'PUT',
    headers: { 'Content-Type': file.type },
    body: file,
  });
  if (!putRes.ok && putRes.status !== 200 && putRes.status !== 204) {
    throw new Error('Receipt upload failed');
  }

  const confirmRes = await fetch('/api/receipts/confirm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRF-Token': getCookie('auth_token'),
    },
    credentials: 'include',
    body: JSON.stringify({
      key: uploadData.key,
      file_name: file.name,
      content_type: file.type,
      size: file.size,
      donation_id: donationId,
    }),
  });
  if (!confirmRes.ok) throw new Error('Failed to confirm receipt');

  const body = await confirmRes.json();
  await deps.db.receipts.put({
    id: crypto.randomUUID(),
    key: uploadData.key,
    file_name: file.name,
    content_type: file.type,
    size: file.size,
    donation_id: donationId,
    uploaded_at: new Date().toISOString(),
    server_id: body && body.id ? body.id : null,
  });
}
