import { renderPaginationHtml, bindPaginationHandlers } from '../../../utils/pagination.js';
import { bindSearchHandlers, bindSortHandlers, paginate, sortItems } from '../../../utils/table.js';

export async function renderCharitiesRoute(deps) {
  const {
    db,
    deleteCharityOnServer,
    escapeHtml,
    getCurrentUserId,
    navigate,
    refreshCharitiesCache,
  } = deps;

  const urlParams = new URLSearchParams(window.location.search);
  let sortField = urlParams.get('sort') || 'name';
  let sortOrder = urlParams.get('order') || 'asc';
  let searchQuery = urlParams.get('q') || '';
  let currentPage = parseInt(urlParams.get('page') || '1', 10);
  const pageSize = 25;

  const root = document.getElementById('route-content') || document.getElementById('app');
  const userId = getCurrentUserId();
  let charities = [];
  try {
    if (userId) {
      charities = await refreshCharitiesCache();
    }
  } catch {
    charities = userId ? await db.charities.where('user_id').equals(userId).toArray() : [];
  }

  const formatAddress = (c) => {
    const parts = [c.street, c.city, c.state, c.zip].map((v) => (v || '').trim()).filter(Boolean);
    return parts.length ? parts.join(', ') : '—';
  };

  // Apply Search
  if (searchQuery) {
    const q = searchQuery.toLowerCase();
    charities = charities.filter((c) => {
      const name = (c.name || '').toLowerCase();
      const ein = (c.ein || '').toLowerCase();
      const category = (c.category || '').toLowerCase();
      const status = (c.status || '').toLowerCase();
      const deductibility = (c.deductibility || '').toLowerCase();
      const address = formatAddress(c).toLowerCase();

      return (
        name.includes(q) ||
        ein.includes(q) ||
        category.includes(q) ||
        status.includes(q) ||
        deductibility.includes(q) ||
        address.includes(q)
      );
    });
  }

  // Apply Sorting
  sortItems(charities, (c, field) => {
    switch (field) {
      case 'ein':
        return c.ein || '';
      case 'category':
        return c.category || '';
      case 'status':
        return c.status || '';
      case 'deductibility':
        return c.deductibility || '';
      case 'address':
        return formatAddress(c);
      default:
        return c.name || '';
    }
  }, sortField, sortOrder);

  // Pagination
  const { currentPage: resolvedPage, totalPages, startIndex, endIndex } = paginate(
    charities.length,
    currentPage,
    pageSize
  );
  currentPage = resolvedPage;
  const totalRecords = charities.length;
  const paginatedCharities = charities.slice(startIndex, endIndex);

  function getSortIcon(field) {
    if (sortField !== field) return '';
    return sortOrder === 'asc' ? ' ↑' : ' ↓';
  }

  root.innerHTML = `
        <div class="mx-auto max-w-7xl space-y-5">
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Charities</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Manage your nonprofit directory.</p>
                </div>
                <div class="flex items-center gap-3">
                    <div class="relative">
                        <input id="search-input" type="text" placeholder="Search..." class="dt-input py-2 pl-3 pr-8 text-sm w-48 sm:w-64" value="${escapeHtml(searchQuery)}" />
                        ${searchQuery ? `<button id="clear-search" class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600">✕</button>` : ''}
                    </div>
                    <button id="btn-new-charity" class="dt-btn-primary whitespace-nowrap">New Charity</button>
                </div>
            </div>
            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                        <thead class="bg-slate-50 dark:bg-slate-800">
                            <tr>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="name">Name${getSortIcon('name')}</th>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="ein">EIN${getSortIcon('ein')}</th>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="category">Category${getSortIcon('category')}</th>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="status">Status${getSortIcon('status')}</th>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="deductibility">Deductibility${getSortIcon('deductibility')}</th>
                                <th class="sortable-header cursor-pointer px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400" data-sort="address">Address${getSortIcon('address')}</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400">Actions</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-800 bg-white dark:bg-slate-900">
                            ${
                              paginatedCharities.length === 0
                                ? '<tr><td colspan="7" class="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">No cached charities found.</td></tr>'
                                : paginatedCharities
                                    .map(
                                      (c) => `
                                <tr class="hover:bg-slate-50 dark:hover:bg-slate-800 cursor-pointer charity-row" data-id="${c.id}">
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
                            `
                                    )
                                    .join('')
                            }
                        </tbody>
                    </table>
                </div>
                <div class="space-y-3 p-4 md:hidden">
                    ${
                      paginatedCharities.length === 0
                        ? '<div class="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 text-sm text-slate-500 dark:text-slate-400">No cached charities found.</div>'
                        : paginatedCharities
                            .map(
                              (c) => `
                        <article class="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 charity-row" data-id="${c.id}">
                            <p class="text-sm font-semibold text-slate-900 dark:text-slate-100">${escapeHtml(c.name)}</p>
                            <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">${escapeHtml(c.ein || 'No EIN')}</p>
                            <p class="mt-2 text-sm text-slate-600 dark:text-slate-300">${escapeHtml(formatAddress(c))}</p>
                            <div class="mt-3 flex gap-2">
                                <button class="edit-charity-btn dt-btn-secondary flex-1 px-3 py-1.5" data-id="${c.id}">Edit</button>
                                <button class="delete-charity-btn dt-btn-danger flex-1 px-3 py-1.5" data-id="${c.id}">Delete</button>
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

  document
    .getElementById('btn-new-charity')
    ?.addEventListener('click', () => navigate('/charities/new'));

  const rerender = () => renderCharitiesRoute(deps);
  bindSearchHandlers({ urlParams, rerender });
  bindSortHandlers({ urlParams, sortField, sortOrder, rerender });
  bindPaginationHandlers({ urlParams, currentPage, totalPages, rerender });

  document.querySelectorAll('.charity-row').forEach((row) => {
    row.addEventListener('click', (e) => {
      if (e.target.closest('button')) return;
      navigate(`/charities/view/${encodeURIComponent(row.dataset.id)}`);
    });
  });

  document.querySelectorAll('.edit-charity-btn').forEach((button) => {
    button.addEventListener('click', (e) => {
      e.stopPropagation();
      navigate(`/charities/edit/${encodeURIComponent(e.currentTarget.dataset.id)}`);
    });
  });

  document.querySelectorAll('.delete-charity-btn').forEach((b) => {
    b.addEventListener('click', async (e) => {
      e.stopPropagation();
      const charityId = e.currentTarget.dataset.id;
      if (!confirm('Are you sure you want to delete this charity?')) return;
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
