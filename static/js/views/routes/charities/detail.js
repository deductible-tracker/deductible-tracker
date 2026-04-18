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
