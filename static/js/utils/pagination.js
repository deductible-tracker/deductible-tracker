/**
 * Shared pagination HTML rendering and event binding.
 *
 * Used by donations and charities list routes to avoid duplicating
 * the same pagination markup and handler wiring.
 */

/**
 * Render pagination HTML for a list view.
 * Returns an empty string when totalPages <= 1.
 */
export function renderPaginationHtml({ currentPage, totalPages, totalRecords, pageSize }) {
  if (totalPages <= 1) return '';

  const startRecord = (currentPage - 1) * pageSize + 1;
  const endRecord = Math.min(currentPage * pageSize, totalRecords);

  const prevDisabled = currentPage === 1;
  const nextDisabled = currentPage === totalPages;
  const disabledClass = 'opacity-50 cursor-not-allowed';

  const pageButtons = Array.from({ length: totalPages }, (_, i) => i + 1)
    .map(
      (p) => `
        <button class="page-btn relative inline-flex items-center px-4 py-2 text-sm font-semibold ${p === currentPage ? 'z-10 bg-indigo-600 text-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600' : 'text-slate-900 dark:text-slate-100 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800'}" data-page="${p}">${p}</button>
      `
    )
    .join('');

  return `
    <div class="flex items-center justify-between border-t border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 sm:px-6 rounded-xl">
      <div class="flex flex-1 justify-between sm:hidden">
        <button id="prev-page-mobile" ${prevDisabled ? 'disabled' : ''} class="dt-btn-secondary px-4 py-2 ${prevDisabled ? disabledClass : ''}">Previous</button>
        <button id="next-page-mobile" ${nextDisabled ? 'disabled' : ''} class="dt-btn-secondary px-4 py-2 ${nextDisabled ? disabledClass : ''}">Next</button>
      </div>
      <div class="hidden sm:flex sm:flex-1 sm:items-center sm:justify-between">
        <div>
          <p class="text-sm text-slate-700 dark:text-slate-300">
            Showing <span class="font-medium">${startRecord}</span> to <span class="font-medium">${endRecord}</span> of <span class="font-medium">${totalRecords}</span> results
          </p>
        </div>
        <div>
          <nav class="isolate inline-flex -space-x-px rounded-md shadow-xs" aria-label="Pagination">
            <button id="prev-page" ${prevDisabled ? 'disabled' : ''} class="relative inline-flex items-center rounded-l-md px-2 py-2 text-slate-400 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 focus:z-20 focus:outline-offset-0 ${prevDisabled ? 'cursor-not-allowed' : ''}">
              <span class="sr-only">Previous</span>
              <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" d="M12.79 5.23a.75.75 0 01-.02 1.06L8.832 10l3.938 3.71a.75.75 0 11-1.04 1.08l-4.5-4.25a.75.75 0 010-1.08l4.5-4.25a.75.75 0 011.06.02z" clip-rule="evenodd"></path></svg>
            </button>
            ${pageButtons}
            <button id="next-page" ${nextDisabled ? 'disabled' : ''} class="relative inline-flex items-center rounded-r-md px-2 py-2 text-slate-400 ring-1 ring-inset ring-slate-300 dark:ring-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 focus:z-20 focus:outline-offset-0 ${nextDisabled ? 'cursor-not-allowed' : ''}">
              <span class="sr-only">Next</span>
              <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd"></path></svg>
            </button>
          </nav>
        </div>
      </div>
    </div>
  `;
}

/**
 * Bind pagination event listeners (page buttons, prev/next, mobile prev/next).
 * @param {object}   params
 * @param {URLSearchParams} params.urlParams
 * @param {number}   params.currentPage
 * @param {number}   params.totalPages
 * @param {Function} params.rerender   - called after updating urlParams
 */
export function bindPaginationHandlers({ urlParams, currentPage, totalPages, rerender }) {
  document.querySelectorAll('.page-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      urlParams.set('page', btn.dataset.page);
      window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
      rerender();
    });
  });

  ['prev-page', 'prev-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage > 1) {
        urlParams.set('page', String(currentPage - 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        rerender();
      }
    });
  });

  ['next-page', 'next-page-mobile'].forEach((id) => {
    document.getElementById(id)?.addEventListener('click', () => {
      if (currentPage < totalPages) {
        urlParams.set('page', String(currentPage + 1));
        window.history.replaceState({}, '', `${window.location.pathname}?${urlParams.toString()}`);
        rerender();
      }
    });
  });
}
