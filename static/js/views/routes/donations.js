import {
  analyzeConfirmedReceipt,
  analyzeUploadedReceipt,
  confirmReceiptUpload,
  mapReceiptSuggestionToDonationDraft,
  uploadReceiptToStorage,
} from '../../services/receipt-upload.js';

export async function renderDonationsRoute(deps) {
  const {
    calculateTaxEstimates,
    db,
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

  // Filters & Sorting state
  const urlParams = new URLSearchParams(window.location.search);
  let currentYear = urlParams.get('year') || 'all';
  let sortField = urlParams.get('sort') || 'date';
  let sortOrder = urlParams.get('order') || 'desc';
  let searchQuery = urlParams.get('q') || '';
  let currentPage = parseInt(urlParams.get('page') || '1', 10);
  const pageSize = 25;

  let donations = userId ? await db.donations.where('user_id').equals(userId).toArray() : [];

  // Get unique years for filter
  const years = [...new Set(donations.map((d) => new Date(d.date).getFullYear()))].sort(
    (a, b) => b - a
  );

  const receipts = await db.receipts.toArray();
  const taxEstimates = await calculateTaxEstimates(
    donations,
    charities,
    receipts,
    getCurrentUser() || {}
  );

  // Apply Year Filter
  if (currentYear !== 'all') {
    const yearNum = parseInt(currentYear, 10);
    donations = donations.filter((d) => new Date(d.date).getFullYear() === yearNum);
  }

  // Apply Search
  if (searchQuery) {
    const q = searchQuery.toLowerCase();
    donations = donations.filter((d) => {
      const charityName = (charityNameMap.get(d.charity_id) || '').toLowerCase();
      const date = (d.date || '').toLowerCase();
      const status = (d.sync_status || 'synced').toLowerCase();
      const category = (d.category || '').toLowerCase();
      const amount = (d.amount != null ? formatCurrency(d.amount) : '').toLowerCase();
      const savings = formatCurrency(taxEstimates.perDonation.get(d.id) || 0).toLowerCase();

      return (
        charityName.includes(q) ||
        date.includes(q) ||
        status.includes(q) ||
        category.includes(q) ||
        amount.includes(q) ||
        savings.includes(q)
      );
    });
  }

  // Apply Sorting
  donations.sort((a, b) => {
    let valA, valB;
    switch (sortField) {
      case 'charity':
        valA = charityNameMap.get(a.charity_id) || '';
        valB = charityNameMap.get(b.charity_id) || '';
        break;
      case 'amount':
        valA = a.amount || 0;
        valB = b.amount || 0;
        break;
      case 'category':
        valA = a.category || '';
        valB = b.category || '';
        break;
      case 'status':
        valA = a.sync_status || '';
        valB = b.sync_status || '';
        break;
      case 'savings':
        valA = taxEstimates.perDonation.get(a.id) || 0;
        valB = taxEstimates.perDonation.get(b.id) || 0;
        break;
      default:
        valA = a.date || '';
        valB = b.date || '';
    }

    if (valA < valB) return sortOrder === 'asc' ? -1 : 1;
    if (valA > valB) return sortOrder === 'asc' ? 1 : -1;
    return 0;
  });

  // Pagination
  const totalRecords = donations.length;
  const totalPages = Math.ceil(totalRecords / pageSize);
  currentPage = Math.max(1, Math.min(currentPage, totalPages || 1));
  const paginatedDonations = donations.slice((currentPage - 1) * pageSize, currentPage * pageSize);

  function getSortIcon(field) {
    if (sortField !== field) return '';
    return sortOrder === 'asc' ? ' ↑' : ' ↓';
  }

  root.innerHTML = `
        <div class="mx-auto max-w-7xl space-y-5">
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Donations</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Add donations, attach receipts immediately, and keep records audit-ready.</p>
                </div>
                <div class="flex items-center gap-3">
                    <div class="relative">
                        <input id="search-input" type="text" placeholder="Search..." class="dt-input py-2 pl-3 pr-8 text-sm w-48 sm:w-64" value="${escapeHtml(searchQuery)}" />
                        ${searchQuery ? `<button id="clear-search" class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600">✕</button>` : ''}
                    </div>
                    <div class="flex items-center gap-2">
                        <select id="year-filter" class="dt-input py-2 px-3 text-sm w-28">
                            <option value="all" ${currentYear === 'all' ? 'selected' : ''}>All Years</option>
                            ${years.map((y) => `<option value="${y}" ${currentYear == y ? 'selected' : ''}>${y}</option>`).join('')}
                        </select>
                    </div>
                    <button id="btn-new-donation" class="dt-btn-primary whitespace-nowrap">New Donation</button>
                </div>
            </div>

            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-700">
                        <thead class="bg-slate-50 dark:bg-slate-700/50">
                            <tr>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="date">
                                  Date${getSortIcon('date')}
                                </th>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="charity">
                                  Charity${getSortIcon('charity')}
                                </th>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="status">
                                  Status${getSortIcon('status')}
                                </th>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="category">
                                  Category${getSortIcon('category')}
                                </th>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="amount">
                                  Amount${getSortIcon('amount')}
                                </th>
                                <th scope="col" class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="savings">
                                  Estimated Tax Savings${getSortIcon('savings')}
                                </th>
                                <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Actions</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-700 bg-white dark:bg-slate-800">
                            ${
                              paginatedDonations.length === 0
                                ? `
                                <tr>
                                    <td colspan="7" class="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">No donations found.</td>
                                </tr>
                            `
                                : paginatedDonations
                                    .map(
                                      (d) => `
                                <tr class="hover:bg-slate-50 dark:bg-slate-700/50/70 cursor-pointer donation-row" data-id="${d.id}">
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
                      paginatedDonations.length === 0
                        ? '<div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 text-sm text-slate-500 dark:text-slate-400">No donations found.</div>'
                        : paginatedDonations
                            .map(
                              (d) => `
                        <article class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 donation-row" data-id="${d.id}">
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
                                <button class="delete-donation-btn dt-btn-danger flex-1 px-3 py-1.5" data-id="${d.id}">Delete</button>
                            </div>
                        </article>
                    `
                            )
                            .join('')
                    }
                </div>
            </div>

            ${
              totalPages > 1
                ? `
              <div class="flex items-center justify-between border-t border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 sm:px-6 rounded-xl">
                <div class="flex flex-1 justify-between sm:hidden">
                  <button id="prev-page-mobile" ${currentPage === 1 ? 'disabled' : ''} class="dt-btn-secondary px-4 py-2 ${currentPage === 1 ? 'opacity-50 cursor-not-allowed' : ''}">Previous</button>
                  <button id="next-page-mobile" ${currentPage === totalPages ? 'disabled' : ''} class="dt-btn-secondary px-4 py-2 ${currentPage === totalPages ? 'opacity-50 cursor-not-allowed' : ''}">Next</button>
                </div>
                <div class="hidden sm:flex sm:flex-1 sm:items-center sm:justify-between">
                  <div>
                    <p class="text-sm text-slate-700 dark:text-slate-300">
                      Showing <span class="font-medium">${(currentPage - 1) * pageSize + 1}</span> to <span class="font-medium">${Math.min(currentPage * pageSize, totalRecords)}</span> of <span class="font-medium">${totalRecords}</span> results
                    </p>
                  </div>
                  <div>
                    <nav class="isolate inline-flex -space-x-px rounded-md shadow-xs" aria-label="Pagination">
                      <button id="prev-page" ${currentPage === 1 ? 'disabled' : ''} class="relative inline-flex items-center rounded-l-md px-2 py-2 text-slate-400 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 focus:z-20 focus:outline-offset-0 ${currentPage === 1 ? 'cursor-not-allowed' : ''}">
                        <span class="sr-only">Previous</span>
                        <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" d="M12.79 5.23a.75.75 0 01-.02 1.06L8.832 10l3.938 3.71a.75.75 0 11-1.04 1.08l-4.5-4.25a.75.75 0 010-1.08l4.5-4.25a.75.75 0 011.06.02z" clip-rule="evenodd"></path></svg>
                      </button>
                      ${Array.from({ length: totalPages }, (_, i) => i + 1)
                        .map(
                          (p) => `
                        <button class="page-btn relative inline-flex items-center px-4 py-2 text-sm font-semibold ${p === currentPage ? 'z-10 bg-indigo-600 text-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600' : 'text-slate-900 dark:text-slate-100 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800'}" data-page="${p}">${p}</button>
                      `
                        )
                        .join('')}
                      <button id="next-page" ${currentPage === totalPages ? 'disabled' : ''} class="relative inline-flex items-center rounded-r-md px-2 py-2 text-slate-400 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 focus:z-20 focus:outline-offset-0 ${currentPage === totalPages ? 'cursor-not-allowed' : ''}">
                        <span class="sr-only">Next</span>
                        <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd"></path></svg>
                      </button>
                    </nav>
                  </div>
                </div>
              </div>
            `
                : ''
            }
        </div>
    `;

  document.getElementById('btn-new-donation')?.addEventListener('click', (e) => {
    e.stopPropagation();
    navigate('/donations/new');
  });

  const searchInput = document.getElementById('search-input');
  let searchTimeout;
  searchInput?.addEventListener('input', (e) => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      urlParams.set('q', e.target.value);
      urlParams.set('page', '1');
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      renderDonationsRoute(deps);
      // Refocus after re-render
      const newSearchInput = document.getElementById('search-input');
      if (newSearchInput) {
        newSearchInput.focus();
        newSearchInput.setSelectionRange(newSearchInput.value.length, newSearchInput.value.length);
      }
    }, 300);
  });

  document.getElementById('clear-search')?.addEventListener('click', () => {
    urlParams.delete('q');
    urlParams.set('page', '1');
    window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
    renderDonationsRoute(deps);
    document.getElementById('search-input')?.focus();
  });

  document.querySelectorAll('.page-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      urlParams.set('page', btn.dataset.page);
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      renderDonationsRoute(deps);
    });
  });

  ['prev-page', 'prev-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage > 1) {
        urlParams.set('page', String(currentPage - 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        renderDonationsRoute(deps);
      }
    });
  });

  ['next-page', 'next-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage < totalPages) {
        urlParams.set('page', String(currentPage + 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        renderDonationsRoute(deps);
      }
    });
  });

  document.querySelectorAll('.sortable-header').forEach((header) => {
    header.addEventListener('click', () => {
      const field = header.dataset.sort;
      if (sortField === field) {
        sortOrder = sortOrder === 'asc' ? 'desc' : 'asc';
      } else {
        sortField = field;
        sortOrder = 'asc';
      }
      urlParams.set('sort', sortField);
      urlParams.set('order', sortOrder);
      urlParams.set('page', '1');
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      renderDonationsRoute(deps);
    });
  });

  document.querySelectorAll('.edit-donation-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      navigate(`/donations/edit/${encodeURIComponent(btn.dataset.id)}`);
    });
  });

  document.querySelectorAll('.delete-donation-btn').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = btn.dataset.id;
      if (!id) return;
      if (confirm('Delete this donation? Associated receipts will also be removed.')) {
        deps.Sync.queueAction('donations', { id }, 'delete');
        renderDonationsRoute(deps);
        await updateTotals();
      }
    });
  });

  document.querySelectorAll('.donation-row').forEach((row) => {
    row.addEventListener('click', (e) => {
      // Don't navigate if clicking a button
      if (e.target.closest('button')) return;
      navigate(`/donations/view/${encodeURIComponent(row.dataset.id)}`);
    });
  });
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

export async function renderDonationViewRoute(donationId, deps) {
  const { db, escapeHtml, formatCurrency, navigate, calculateTaxEstimates, getCurrentUser } = deps;
  const donation = await db.donations.get(donationId);
  if (!donation) {
    alert('Donation not found');
    navigate('/donations');
    return;
  }
  const charity = await db.charities.get(donation.charity_id);
  const receipts = await db.receipts.where('donation_id').equals(donationId).toArray();
  const taxEstimates = await calculateTaxEstimates(
    [donation],
    charity ? [charity] : [],
    receipts,
    getCurrentUser() || {}
  );

  const root = document.getElementById('route-content') || document.getElementById('app');
  root.innerHTML = `
    <div class="mx-auto max-w-3xl space-y-5">
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Donation Details</h1>
          <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">View information and receipts for this donation.</p>
        </div>
        <div class="flex gap-2">
          <button id="btn-edit-donation" class="dt-btn-primary">Edit</button>
          <button id="btn-back-donations" class="dt-btn-secondary">Back</button>
        </div>
      </div>
      <div class="dt-panel p-5 sm:p-6 space-y-6">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Date</label>
            <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(donation.date)}</p>
          </div>
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Charity</label>
            <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity?.name || 'Unknown charity')}</p>
          </div>
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Category</label>
            <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(donation.category || '')}</p>
          </div>
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Amount</label>
            <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${donation.amount ? formatCurrency(donation.amount) : '$0.00'}</p>
          </div>
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Est. Tax Savings</label>
            <p class="mt-1 text-sm font-medium text-emerald-700 dark:text-emerald-300">${formatCurrency(taxEstimates.totalEstimated)}</p>
          </div>
        </div>
        ${
          donation.notes
            ? `
          <div>
            <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Notes</label>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300 whitespace-pre-wrap">${escapeHtml(donation.notes)}</p>
          </div>
        `
            : ''
        }
        
        <div class="border-t border-slate-200 dark:border-slate-700 pt-6">
          <h3 class="text-sm font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400 mb-4">Receipts</h3>
          <div id="receipts-list" class="space-y-2">
            ${
              receipts.length === 0
                ? '<p class="text-sm text-slate-500 dark:text-slate-400">No receipts attached.</p>'
                : receipts
                    .map(
                      (r) => `
              <div class="flex items-center justify-between rounded-xl border border-slate-200 dark:border-slate-700 p-3 bg-white dark:bg-slate-800">
                <div class="min-w-0">
                  <p class="truncate text-sm font-medium text-slate-900 dark:text-slate-100">${escapeHtml(r.file_name || 'Receipt')}</p>
                  <p class="text-xs text-slate-500 dark:text-slate-400">${r.uploaded_at ? new Date(r.uploaded_at).toLocaleDateString() : ''}</p>
                </div>
                <button class="preview-receipt-btn dt-btn-secondary px-3 py-1.5" data-key="${escapeHtml(r.key)}">Preview</button>
              </div>
            `
                    )
                    .join('')
            }
          </div>
        </div>
      </div>
    </div>
  `;

  document
    .getElementById('btn-back-donations')
    ?.addEventListener('click', () => navigate('/donations'));
  document
    .getElementById('btn-edit-donation')
    ?.addEventListener('click', () =>
      navigate(`/donations/edit/${encodeURIComponent(donationId)}`)
    );

  document.querySelectorAll('.preview-receipt-btn').forEach((btn) => {
    btn.addEventListener('click', async (e) => {
      const key = e.currentTarget.dataset.key;
      try {
        const url = await getReceiptDownloadUrlRoute(key, deps);
        window.open(url, '_blank');
      } catch (err) {
        alert('Failed to preview receipt');
      }
    });
  });
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
                            <label class="dt-label uppercase tracking-wide text-slate-500 text-[10px] font-bold">Upload Receipts</label>
                            <div id="receipt-dropzone" class="mt-1 flex justify-center px-6 pt-5 pb-6 border-2 border-slate-300 dark:border-slate-700 border-dashed rounded-xl cursor-pointer hover:border-indigo-500 dark:hover:border-indigo-400">
                                <div class="space-y-1 text-center">
                                  <svg class="mx-auto h-12 w-12 text-slate-400" stroke="currentColor" fill="none" viewBox="0 0 48 48" aria-hidden="true">
                                    <path d="M28 8H12a4 4 0 00-4 4v20m32-12v8m0 0v8a4 4 0 00-4 4H12a4 4 0 00-4-4v-4m32-4l-3.172-3.172a4 4 0 00-5.656 0L28 28M8 32l9.172-9.172a4 4 0 015.656 0L28 28m0 0l4 4m4-24h8m-4-4v8m-12 4h.02" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
                                  </svg>
                                  <div class="flex flex-col items-center text-sm text-slate-600 dark:text-slate-400">
                                    <label for="donation-receipts" class="relative cursor-pointer bg-white dark:bg-slate-800 rounded-md font-medium text-indigo-600 dark:text-indigo-400 hover:text-indigo-500 focus-within:outline-hidden">
                                      <span>Upload or drag and drop</span>
                                      <input id="donation-receipts" name="donation-receipts" type="file" multiple 
                                        accept=".pdf,.docx,.doc,.pptx,.ppt,.xlsx,.csv,.txt,.epub,.xml,.rtf,.odt,.bib,.fb2,.ipynb,.tex,.opml,.1,.man,image/jpeg,image/png,image/avif,image/tiff,image/gif,image/heic,image/heif,image/bmp,image/webp" 
                                        class="sr-only" />
                                    </label>
                                  </div>
                                  <p class="text-xs text-slate-500 dark:text-slate-400">Supported file types include: PDF, DOCX, HEIC/HEIF, JPG, PNG</p>
                                </div>
                            </div>
                        <p id="donation-receipts-status" class="mt-2 text-xs text-slate-500 dark:text-slate-400">Selecting a receipt uploads it immediately and applies any available OCR prefill.</p>
                        <div id="donation-receipts-list" class="mt-3 space-y-2"></div>
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
  const todayIso = new Date().toISOString().split('T')[0];
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
  const dateInput = document.getElementById('donation-date');
  const receiptInput = document.getElementById('donation-receipts');
  const dropzone = document.getElementById('receipt-dropzone');
  const receiptStatus = document.getElementById('donation-receipts-status');
  const receiptList = document.getElementById('donation-receipts-list');
  const draftReceipts = [];

  function parseDonationAmount(rawValue) {
    if (!rawValue) return null;
    const parsed = Number.parseFloat(rawValue);
    return Number.isFinite(parsed) ? parsed : Number.NaN;
  }

  function validateDonationSubmission({
    date,
    charityName,
    charityId,
    category,
    amountRaw,
    amount,
  }) {
    if (!date || !charityName) {
      return 'Please provide date and charity name';
    }

    if (amountRaw && !Number.isFinite(amount)) {
      return 'Please enter a valid amount';
    }

    if (amount != null && Number.isFinite(amount) && amount < 0) {
      return 'Donation amount cannot be negative';
    }

    if (category === 'money' && (!Number.isFinite(amount) || amount <= 0)) {
      return 'Money donations require a positive amount';
    }

    if (!charityId) {
      return 'Please select or create a valid charity before saving this donation';
    }

    return null;
  }

  // Load existing receipts if in edit mode
  if (isEditMode) {
    const existingReceipts = await db.receipts
      .where('donation_id')
      .equals(existingDonation.id)
      .toArray();
    for (const r of existingReceipts) {
      draftReceipts.push({
        local_id: r.id,
        server_id: r.id,
        donation_id: r.donation_id,
        file_name: r.file_name,
        content_type: r.content_type,
        size: r.size,
        key: r.key,
        stage: 'attached',
        analysis: r.ocr_status ? { status: r.ocr_status, suggestion: null } : null,
      });
    }
  }

  function setReceiptStatus(message, tone = 'muted') {
    if (!receiptStatus) return;
    const toneClass =
      tone === 'error'
        ? 'text-rose-600 dark:text-rose-300'
        : tone === 'success'
          ? 'text-emerald-600 dark:text-emerald-300'
          : 'text-slate-500 dark:text-slate-400';
    receiptStatus.className = `mt-2 text-xs ${toneClass}`;
    receiptStatus.textContent = message;
  }

  function _findReceiptEntry(localId) {
    return draftReceipts.find((entry) => entry.local_id === localId) || null;
  }

  function describeReceiptStage(entry) {
    switch (entry.stage) {
      case 'uploading':
        return 'Uploading';
      case 'attaching':
        return 'Attaching to donation';
      case 'analyzing':
        return '<div class="flex items-center gap-1.5"><svg class="animate-spin h-3 w-3 text-indigo-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path></svg><span>Analyzing...</span></div>';
      case 'attached':
        return 'Attached';
      case 'pending-save':
        return 'Uploaded';
      case 'error':
        return 'Needs attention';
      default:
        return 'Queued';
    }
  }

  function summarizeReceiptSuggestion(entry) {
    const suggestion = entry.analysis && entry.analysis.suggestion;
    if (!suggestion) {
      return entry.warning || (entry.analysis && entry.analysis.warning) || '';
    }

    const summary = [];
    if (suggestion.organizationName) summary.push(suggestion.organizationName);
    if (suggestion.dateOfDonation) summary.push(suggestion.dateOfDonation);
    if (suggestion.amountUsd != null) summary.push(`$${Number(suggestion.amountUsd).toFixed(2)}`);
    if (suggestion.itemName) summary.push(suggestion.itemName);
    return summary.join(' • ');
  }

  function renderDraftReceipts() {
    if (!receiptList) return;

    if (draftReceipts.length === 0) {
      receiptList.innerHTML =
        '<div class="rounded-xl border border-dashed border-slate-200 dark:border-slate-700 px-3 py-3 text-xs text-slate-500 dark:text-slate-400">No receipts selected yet.</div>';
      return;
    }

    receiptList.innerHTML = draftReceipts
      .map((entry) => {
        const note = summarizeReceiptSuggestion(entry);
        const description = describeReceiptStage(entry);
        const fileContent = entry.key
          ? `<button type="button" class="preview-receipt-btn text-sm font-medium text-indigo-600 dark:text-indigo-400 hover:underline truncate block w-full text-left" data-key="${escapeHtml(entry.key)}">${escapeHtml(entry.file_name || 'Receipt')}</button>`
          : `<p class="truncate text-sm font-medium text-slate-900 dark:text-slate-100">${escapeHtml(entry.file_name || 'Receipt')}</p>`;

        return `
          <div class="rounded-xl border border-slate-200 dark:border-slate-700 px-3 py-3 bg-white dark:bg-slate-800">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0 flex-1">
                ${fileContent}
                <div class="mt-1 text-xs text-slate-500 dark:text-slate-400">${description}</div>
                ${note ? `<p class="mt-1 text-xs text-slate-500 dark:text-slate-400">${escapeHtml(note)}</p>` : ''}
              </div>
              <div class="flex items-center gap-2">
                <button type="button" class="delete-receipt-btn text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 p-1" data-local-id="${escapeHtml(entry.local_id)}" title="Remove receipt">
                  <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
                ${entry.stage === 'error' ? '<span class="text-xs font-medium text-rose-600 dark:text-rose-300">Error</span>' : ''}
              </div>
            </div>
          </div>
        `;
      })
      .join('');

    receiptList.querySelectorAll('.preview-receipt-btn').forEach((btn) => {
      btn.addEventListener('click', async (e) => {
        const key = e.currentTarget.dataset.key;
        try {
          const url = await getReceiptDownloadUrlRoute(key, deps);
          window.open(url, '_blank');
        } catch (err) {
          alert('Failed to preview receipt');
        }
      });
    });

    receiptList.querySelectorAll('.delete-receipt-btn').forEach((btn) => {
      btn.addEventListener('click', async (e) => {
        const localId = e.currentTarget.dataset.localId;
        const entry = _findReceiptEntry(localId);
        if (!entry) return;

        if (
          confirm(
            'Are you sure you want to delete this receipt? This will remove it from the donation and delete the file.'
          )
        ) {
          try {
            if (entry.server_id) {
              // It's already synced or attached, queue a delete on server
              deps.Sync.queueAction('receipts', { id: entry.server_id }, 'delete');
            }

            // Remove from local draft list
            const idx = draftReceipts.findIndex((r) => r.local_id === localId);
            if (idx !== -1) draftReceipts.splice(idx, 1);

            renderDraftReceipts();
            setReceiptStatus('Receipt removed.', 'success');
          } catch (err) {
            console.error('Failed to delete receipt', err);
            alert('Failed to delete receipt.');
          }
        }
      });
    });
  }

  function applyReceiptAnalysis(analysis) {
    const patch = mapReceiptSuggestionToDonationDraft(analysis);
    if (!patch) return;

    if (patch.date && (!dateInput.value || (!isEditMode && dateInput.value === todayIso))) {
      dateInput.value = patch.date;
    }
    if (patch.charityName && !charityInput.value.trim()) {
      charityInput.value = patch.charityName;
    }
    if (
      patch.category &&
      (!isEditMode || !existingDonation.category || !existingDonation.amount) &&
      categorySelect.value !== patch.category
    ) {
      categorySelect.value = patch.category;
      categorySelect.dispatchEvent(new Event('change'));
    }
    if (patch.amount != null && !amountInput.value) {
      amountInput.value = Number(patch.amount).toFixed(2);
    }
    if (patch.itemName && !valuationInput.value.trim()) {
      valuationInput.value = patch.itemName;
      if (patch.category === 'items') {
        valuationFields?.classList.remove('hidden');
      }
    }
  }

  function upsertDraftReceipt(entryPatch) {
    const existingIndex = draftReceipts.findIndex(
      (entry) => entry.local_id === entryPatch.local_id
    );
    if (existingIndex >= 0) {
      draftReceipts[existingIndex] = { ...draftReceipts[existingIndex], ...entryPatch };
    } else {
      draftReceipts.push(entryPatch);
    }
    renderDraftReceipts();
  }

  async function persistReceiptLocally(confirmedReceipt, analysis) {
    await db.receipts.put({
      id: confirmedReceipt.id,
      key: confirmedReceipt.key,
      file_name: confirmedReceipt.file_name,
      content_type: confirmedReceipt.content_type,
      size: confirmedReceipt.size,
      donation_id: confirmedReceipt.donation_id,
      uploaded_at: new Date().toISOString(),
      ocr_status: analysis && analysis.status ? analysis.status : null,
      ocr_text: analysis && analysis.ocrText ? analysis.ocrText : null,
    });
  }

  async function analyzeAndApplyDraftReceipt(entry) {
    try {
      const analysis = entry.server_id
        ? await analyzeConfirmedReceipt(entry.server_id)
        : await analyzeUploadedReceipt(entry);
      applyReceiptAnalysis(analysis);
      return analysis;
    } catch (err) {
      return {
        status: 'failed',
        warning: err && err.message ? err.message : 'Receipt analysis failed.',
        suggestion: null,
      };
    }
  }

  async function attachPendingReceiptsToDonation(donationId) {
    let failures = 0;
    for (const entry of draftReceipts) {
      if (!entry.key || entry.server_id || entry.stage === 'error') continue;
      upsertDraftReceipt({ local_id: entry.local_id, stage: 'attaching', warning: null });
      try {
        const confirmed = await confirmReceiptUpload(entry, donationId);
        // Do not run OCR analysis here; it's already done during upload
        const analysis = entry.analysis;
        await persistReceiptLocally(confirmed, analysis);
        upsertDraftReceipt({
          local_id: entry.local_id,
          donation_id: donationId,
          server_id: confirmed.id,
          stage: 'attached',
          analysis,
          warning: analysis && analysis.warning ? analysis.warning : null,
        });
      } catch (err) {
        failures += 1;
        upsertDraftReceipt({
          local_id: entry.local_id,
          stage: 'error',
          warning: err && err.message ? err.message : 'Failed to attach receipt.',
        });
      }
    }
    renderDraftReceipts();
    return failures;
  }

  async function handleReceiptSelection(file) {
    const localId = crypto.randomUUID();
    const isFirstReceipt = draftReceipts.length === 0 && !isEditMode;

    upsertDraftReceipt({
      local_id: localId,
      file_name: file.name,
      content_type: file.type,
      size: file.size,
      stage: 'uploading',
      warning: null,
      analysis: null,
    });

    try {
      const uploaded = await uploadReceiptToStorage(file);
      const canAttachImmediately = isEditMode && existingDonation.sync_status === 'synced';
      upsertDraftReceipt({
        local_id: localId,
        ...uploaded,
        stage: canAttachImmediately ? 'attaching' : 'analyzing', // will update logic below
      });

      if (canAttachImmediately) {
        const confirmed = await confirmReceiptUpload(uploaded, existingDonation.id);
        // Analysis only for first receipt on NEW donation
        const skipAnalysis = isEditMode || !isFirstReceipt;
        const analysis = skipAnalysis
          ? { status: 'skipped', suggestion: null }
          : await analyzeAndApplyDraftReceipt({ ...uploaded, server_id: confirmed.id });

        await persistReceiptLocally(confirmed, analysis);
        upsertDraftReceipt({
          local_id: localId,
          donation_id: existingDonation.id,
          server_id: confirmed.id,
          stage: 'attached',
          analysis,
          warning: analysis && analysis.warning ? analysis.warning : null,
        });
      } else {
        const skipAnalysis = isEditMode || !isFirstReceipt;
        const analysis = skipAnalysis
          ? { status: 'skipped', suggestion: null }
          : await analyzeAndApplyDraftReceipt(uploaded);

        upsertDraftReceipt({
          local_id: localId,
          stage: 'pending-save',
          analysis,
          warning: analysis && analysis.warning ? analysis.warning : null,
        });
      }

      setReceiptStatus('Receipt uploaded successfully.', 'success');
    } catch (err) {
      upsertDraftReceipt({
        local_id: localId,
        stage: 'error',
        warning: err && err.message ? err.message : 'Failed to upload receipt.',
      });
      setReceiptStatus('Receipt upload failed.', 'error');
    }
  }

  dropzone?.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropzone.classList.add('border-indigo-500', 'bg-indigo-50', 'dark:bg-indigo-900/20');
  });

  dropzone?.addEventListener('dragleave', (e) => {
    e.preventDefault();
    dropzone.classList.remove('border-indigo-500', 'bg-indigo-50', 'dark:bg-indigo-900/20');
  });

  dropzone?.addEventListener('drop', async (e) => {
    e.preventDefault();
    dropzone.classList.remove('border-indigo-500', 'bg-indigo-50', 'dark:bg-indigo-900/20');
    const files = Array.from(e.dataTransfer.files);
    if (files.length === 0) return;
    setReceiptStatus(`Uploading ${files.length} receipt${files.length === 1 ? '' : 's'}...`);
    for (const file of files) {
      await handleReceiptSelection(file);
    }
  });

  dropzone?.addEventListener('click', (e) => {
    if (e.target.closest('span')) return; // ignore if clicking "Upload" link/input
    receiptInput.click();
  });

  receiptInput?.addEventListener('change', async () => {
    const files = Array.from(receiptInput.files || []);
    if (files.length === 0) return;
    receiptInput.value = '';
    setReceiptStatus(`Uploading ${files.length} receipt${files.length === 1 ? '' : 's'}...`);
    for (const file of files) {
      await handleReceiptSelection(file);
    }
  });

  renderDraftReceipts();

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

    const amountRaw = document.getElementById('donation-amount').value.trim();
    const amount = parseDonationAmount(amountRaw);

    const year = new Date(date).getFullYear();
    try {
      let charityId = charity_id || (isEditMode ? existingDonation.charity_id : '') || '';
      if (!charityId) {
        try {
          const resp = await createOrGetCharityOnServer(charity_name, charity_ein || null);
          const charity = resp && resp.charity ? resp.charity : null;
          if (charity && charity.id) {
            charityId = charity.id;
            charityIdInput.value = charityId;
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
        } catch (err) {
          console.error('Failed to resolve charity before saving donation', err);
        }
      }

      const validationError = validateDonationSubmission({
        date,
        charityName: charity_name,
        charityId,
        category,
        amountRaw,
        amount,
      });
      if (validationError) {
        alert(validationError);
        return;
      }

      const payload = {
        date,
        charity_name,
        charity_id: charityId,
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
            charity_id: charityId,
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
            charity_id: charityId,
            notes: notes || null,
            category,
            amount,
            sync_status: 'pending',
          };
          await Sync.queueAction('donations', donation, 'create');
        }
      }

      const pendingReceipts = draftReceipts.filter((entry) => entry.key && !entry.server_id);
      if (pendingReceipts.length > 0 && donation.sync_status === 'synced') {
        const failures = await attachPendingReceiptsToDonation(donation.id);
        if (failures > 0) {
          alert(
            `Donation saved, but ${failures} receipt${failures === 1 ? '' : 's'} failed to attach.`
          );
        }
      } else if (pendingReceipts.length > 0) {
        // Queue these for later sync
        for (const entry of pendingReceipts) {
          const queuedReceiptId = entry.queued_id || crypto.randomUUID();
          upsertDraftReceipt({
            local_id: entry.local_id,
            queued_id: queuedReceiptId,
            donation_id: donation.id,
            stage: 'queued',
            warning: null,
          });
          await deps.Sync.queueAction(
            'receipts',
            {
              id: queuedReceiptId,
              donation_id: donation.id,
              key: entry.key,
              file_name: entry.file_name,
              content_type: entry.content_type,
              size: entry.size,
            },
            'create'
          );
        }
      }

      await updateTotals();
      navigate('/donations');
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

export async function renderReceiptPageRoute(_donationId, deps) {
  // This route is no longer used but we keep the export for now to avoid breaking imports
  // until app.js is fully updated and verified.
  await deps.navigate('/donations');
}
