import { renderPaginationHtml, bindPaginationHandlers } from '../../../utils/pagination.js';
import { bindSearchHandlers, bindSortHandlers, paginate, sortItems } from '../../../utils/table.js';

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
  sortItems(donations, (d, field) => {
    switch (field) {
      case 'charity':
        return charityNameMap.get(d.charity_id) || '';
      case 'amount':
        return d.amount || 0;
      case 'category':
        return d.category || '';
      case 'status':
        return d.sync_status || '';
      case 'savings':
        return taxEstimates.perDonation.get(d.id) || 0;
      default:
        return d.date || '';
    }
  }, sortField, sortOrder);

  // Pagination
  const { currentPage: resolvedPage, totalPages, startIndex, endIndex } = paginate(
    donations.length,
    currentPage,
    pageSize
  );
  currentPage = resolvedPage;
  const totalRecords = donations.length;
  const paginatedDonations = donations.slice(startIndex, endIndex);

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

            ${renderPaginationHtml({ currentPage, totalPages, totalRecords, pageSize })}
        </div>
    `;

  document.getElementById('btn-new-donation')?.addEventListener('click', (e) => {
    e.stopPropagation();
    navigate('/donations/new');
  });

  const rerender = () => renderDonationsRoute(deps);
  bindSearchHandlers({ urlParams, rerender });
  bindSortHandlers({ urlParams, sortField, sortOrder, rerender });
  bindPaginationHandlers({ urlParams, currentPage, totalPages, rerender });

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
