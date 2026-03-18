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
  charities.sort((a, b) => {
    let valA, valB;
    switch (sortField) {
      case 'ein':
        valA = a.ein || '';
        valB = b.ein || '';
        break;
      case 'category':
        valA = a.category || '';
        valB = b.category || '';
        break;
      case 'status':
        valA = a.status || '';
        valB = b.status || '';
        break;
      case 'deductibility':
        valA = a.deductibility || '';
        valB = b.deductibility || '';
        break;
      case 'address':
        valA = formatAddress(a);
        valB = formatAddress(b);
        break;
      default:
        valA = a.name || '';
        valB = b.name || '';
    }

    if (valA < valB) return sortOrder === 'asc' ? -1 : 1;
    if (valA > valB) return sortOrder === 'asc' ? 1 : -1;
    return 0;
  });

  // Pagination
  const totalRecords = charities.length;
  const totalPages = Math.ceil(totalRecords / pageSize);
  currentPage = Math.max(1, Math.min(currentPage, totalPages || 1));
  const paginatedCharities = charities.slice((currentPage - 1) * pageSize, currentPage * pageSize);

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
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-700">
                        <thead class="bg-slate-50 dark:bg-slate-700/50">
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
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-700 bg-white dark:bg-slate-800">
                            ${
                              paginatedCharities.length === 0
                                ? '<tr><td colspan="7" class="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">No cached charities found.</td></tr>'
                                : paginatedCharities
                                    .map(
                                      (c) => `
                                <tr class="hover:bg-slate-50 dark:bg-slate-700/50/70 cursor-pointer charity-row" data-id="${c.id}">
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
                        ? '<div class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 text-sm text-slate-500 dark:text-slate-400">No cached charities found.</div>'
                        : paginatedCharities
                            .map(
                              (c) => `
                        <article class="rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-4 charity-row" data-id="${c.id}">
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

  document
    .getElementById('btn-new-charity')
    ?.addEventListener('click', () => navigate('/charities/new'));

  const searchInput = document.getElementById('search-input');
  let searchTimeout;
  searchInput?.addEventListener('input', (e) => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      urlParams.set('q', e.target.value);
      urlParams.set('page', '1');
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      renderCharitiesRoute(deps);
      document.getElementById('search-input')?.focus();
      const input = document.getElementById('search-input');
      if (input) input.setSelectionRange(input.value.length, input.value.length);
    }, 300);
  });

  document.getElementById('clear-search')?.addEventListener('click', () => {
    urlParams.delete('q');
    urlParams.set('page', '1');
    window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
    renderCharitiesRoute(deps);
  });

  document.querySelectorAll('.page-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      urlParams.set('page', btn.dataset.page);
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      renderCharitiesRoute(deps);
    });
  });

  ['prev-page', 'prev-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage > 1) {
        urlParams.set('page', String(currentPage - 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        renderCharitiesRoute(deps);
      }
    });
  });

  ['next-page', 'next-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage < totalPages) {
        urlParams.set('page', String(currentPage + 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        renderCharitiesRoute(deps);
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
      renderCharitiesRoute(deps);
    });
  });

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

export async function renderCharityViewRoute(charityId, deps) {
  const { db, escapeHtml, navigate } = deps;
  const root = document.getElementById('route-content') || document.getElementById('app');
  const charity = await db.charities.get(charityId);

  if (!charity) {
    alert('Charity not found');
    await navigate('/charities');
    return;
  }

  const formatAddress = (c) => {
    const parts = [c.street, c.city, c.state, c.zip].map((v) => (v || '').trim()).filter(Boolean);
    return parts.length ? parts.join(', ') : 'No address on file';
  };

  root.innerHTML = `
        <div class="mx-auto max-w-4xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">${escapeHtml(charity.name)}</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Detailed charity information and history.</p>
                </div>
                <div class="flex gap-2">
                  <button id="btn-back-charities" class="dt-btn-secondary">Back</button>
                  <button id="btn-edit-charity" class="dt-btn-primary" data-id="${charity.id}">Edit</button>
                </div>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-3 gap-5">
              <div class="md:col-span-2 space-y-5">
                <div class="dt-panel p-6">
                  <h3 class="text-lg font-medium text-slate-900 dark:text-slate-100 mb-4 border-b border-slate-100 dark:border-slate-700 pb-2">Organization Details</h3>
                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-6">
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">EIN</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity.ein || '—')}</p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Nonprofit Type</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity.nonprofit_type || '—')}</p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Category</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity.category || '—')}</p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Status</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">
                        <span class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">${escapeHtml(charity.status || 'Active')}</span>
                      </p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Classification</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity.classification || '—')}</p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Deductibility</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${escapeHtml(charity.deductibility || '—')}</p>
                    </div>
                  </div>
                </div>

                <div class="dt-panel p-6">
                  <h3 class="text-lg font-medium text-slate-900 dark:text-slate-100 mb-4 border-b border-slate-100 dark:border-slate-700 pb-2">Location</h3>
                  <div>
                    <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Address</label>
                    <p class="mt-1 text-sm text-slate-900 dark:text-slate-100 whitespace-pre-wrap">${escapeHtml(formatAddress(charity))}</p>
                  </div>
                </div>
              </div>

              <div class="space-y-5">
                <div class="dt-panel p-6">
                  <h3 class="text-lg font-medium text-slate-900 dark:text-slate-100 mb-4 border-b border-slate-100 dark:border-slate-700 pb-2">Sync Information</h3>
                  <div class="space-y-4">
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">System ID</label>
                      <p class="mt-1 text-xs text-slate-400 font-mono break-all">${charity.id}</p>
                    </div>
                    <div>
                      <label class="text-xs font-semibold text-slate-500 uppercase tracking-wider">Last Cached</label>
                      <p class="mt-1 text-sm text-slate-900 dark:text-slate-100">${charity.cached_at ? new Date(charity.cached_at).toLocaleString() : 'Never'}</p>
                    </div>
                  </div>
                </div>
              </div>
            </div>
        </div>
    `;

  document
    .getElementById('btn-back-charities')
    ?.addEventListener('click', () => navigate('/charities'));
  document.getElementById('btn-edit-charity')?.addEventListener('click', (e) => {
    navigate(`/charities/edit/${encodeURIComponent(e.currentTarget.dataset.id)}`);
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

  document
    .getElementById('btn-back-charities')
    ?.addEventListener('click', () => navigate('/charities'));

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
      if (!query || query.length < 2) {
        suggestions.innerHTML = '';
        suggestions.classList.add('hidden');
        return;
      }
      searchTimer = setTimeout(async () => {
        try {
          const { res, data } = await apiJson(
            `/api/charities/search?q=${encodeURIComponent(query)}`
          );
          if (!res.ok || !data || !Array.isArray(data.results) || data.results.length === 0) {
            suggestions.innerHTML = '';
            suggestions.classList.add('hidden');
            return;
          }
          suggestions.innerHTML = data.results
            .slice(0, 7)
            .map(
              (item) => `
                        <button type="button" class="charity-suggestion-item w-full border-b border-slate-100 dark:border-slate-700 p-2 text-left last:border-b-0 hover:bg-slate-50 dark:bg-slate-700/50" data-name="${escapeHtml(item.name || '')}" data-ein="${escapeHtml(item.ein || '')}" data-location="${escapeHtml(item.location || '')}">
                            <div class="text-sm font-medium text-slate-800">${escapeHtml(item.name || '')}</div>
                            <div class="text-xs text-slate-500 dark:text-slate-400">${escapeHtml(item.ein || '')}${item.location ? ` • ${escapeHtml(item.location)}` : ''}</div>
                        </button>
                    `
            )
            .join('');
          suggestions.classList.remove('hidden');

          suggestions.querySelectorAll('.charity-suggestion-item').forEach((button) => {
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
              } catch (detailErr) {
                console.warn('Charity EIN lookup failed', detailErr);
              }
            });
          });
        } catch (err) {
          console.error('Charity typeahead failed', err);
          suggestions.classList.add('hidden');
        }
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
    if (!name) {
      alert('Name required');
      return;
    }
    if (!isEditMode && (!city || !state)) {
      alert('City and State are required');
      return;
    }

    try {
      let charity = null;
      if (isEditMode && existingCharity?.id) {
        const updatePayload = {
          name,
          ein: ein || null,
          category,
          status,
          classification,
          nonprofit_type,
          deductibility,
          street,
          city,
          state,
          zip,
        };
        const resp = await updateCharityOnServer(existingCharity.id, updatePayload);
        charity =
          resp && resp.charity ? resp.charity : { id: existingCharity.id, ...updatePayload };
      } else {
        const resp = await createOrGetCharityOnServer({
          name,
          ein: ein || null,
          category,
          status,
          classification,
          nonprofit_type,
          deductibility,
          street,
          city,
          state,
          zip,
        });
        charity = resp && resp.charity ? resp.charity : null;
        if (!charity || !charity.id) throw new Error('Failed to create charity');
      }

      if (charity && userId) {
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
  root.innerHTML = buildCharityFormHtmlRoute(
    { title: 'New Charity', isEditMode: false, existing: null },
    deps
  );
  await bindCharityFormHandlersRoute({ userId, existingCharity: null, isEditMode: false }, deps);
}

export async function renderCharityEditRoute(charityId, deps) {
  const root = document.getElementById('route-content') || document.getElementById('app');
  const userId = deps.getCurrentUserId();
  let charities = [];
  try {
    charities = userId ? await deps.db.charities.where('user_id').equals(userId).toArray() : [];
  } catch {
    /* ignore */
  }
  const existing = charities.find((c) => c.id === charityId);
  if (!existing) {
    alert('Charity not found');
    await deps.navigate('/charities');
    return;
  }
  root.innerHTML = buildCharityFormHtmlRoute(
    { title: 'Edit Charity', isEditMode: true, existing },
    deps
  );
  await bindCharityFormHandlersRoute({ userId, existingCharity: existing, isEditMode: true }, deps);
}
