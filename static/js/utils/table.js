/**
 * Shared table interaction helpers: search input with debounce,
 * sortable column headers, and URL-based state management.
 *
 * Used by donations and charities list routes.
 */

/**
 * Bind the search input with debounced re-render.
 * @param {object}   params
 * @param {URLSearchParams} params.urlParams
 * @param {Function} params.rerender
 * @param {number}   [params.debounceMs=300]
 */
export function bindSearchHandlers({ urlParams, rerender, debounceMs = 300 }) {
  const searchInput = document.getElementById('search-input');
  let searchTimeout;
  searchInput?.addEventListener('input', (e) => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      urlParams.set('q', e.target.value);
      urlParams.set('page', '1');
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      rerender();
      const newInput = document.getElementById('search-input');
      if (newInput) {
        newInput.focus();
        newInput.setSelectionRange(newInput.value.length, newInput.value.length);
      }
    }, debounceMs);
  });

  document.getElementById('clear-search')?.addEventListener('click', () => {
    urlParams.delete('q');
    urlParams.set('page', '1');
    window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
    rerender();
    document.getElementById('search-input')?.focus();
  });
}

/**
 * Bind sortable column header clicks.
 * Toggles order when the same column is clicked, resets to 'asc' for a new column.
 * @param {object}   params
 * @param {URLSearchParams} params.urlParams
 * @param {string}   params.sortField  - current sort field
 * @param {string}   params.sortOrder  - current sort order ('asc'|'desc')
 * @param {Function} params.rerender
 */
export function bindSortHandlers({ urlParams, sortField, sortOrder, rerender }) {
  document.querySelectorAll('.sortable-header').forEach((header) => {
    header.addEventListener('click', () => {
      const field = header.dataset.sort;
      let newOrder;
      if (sortField === field) {
        newOrder = sortOrder === 'asc' ? 'desc' : 'asc';
      } else {
        newOrder = 'asc';
      }
      urlParams.set('sort', field);
      urlParams.set('order', newOrder);
      urlParams.set('page', '1');
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      rerender();
    });
  });
}

/**
 * Compute pagination slice boundaries from a total record count.
 * @returns {{ currentPage: number, totalPages: number, startIndex: number, endIndex: number }}
 */
export function paginate(totalRecords, rawPage, pageSize) {
  const totalPages = Math.ceil(totalRecords / pageSize);
  const currentPage = Math.max(1, Math.min(rawPage, totalPages || 1));
  return {
    currentPage,
    totalPages,
    startIndex: (currentPage - 1) * pageSize,
    endIndex: currentPage * pageSize,
  };
}

/**
 * Generic sort comparator for list views.
 * @param {Array} items
 * @param {Function} valueExtractor - (item, field) => comparable value
 * @param {string} sortField
 * @param {string} sortOrder - 'asc' | 'desc'
 */
export function sortItems(items, valueExtractor, sortField, sortOrder) {
  return items.sort((a, b) => {
    const valA = valueExtractor(a, sortField);
    const valB = valueExtractor(b, sortField);
    if (valA < valB) return sortOrder === 'asc' ? -1 : 1;
    if (valA > valB) return sortOrder === 'asc' ? 1 : -1;
    return 0;
  });
}
