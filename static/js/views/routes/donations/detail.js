async function getReceiptDownloadUrl(key, deps) {
  const { getCookie } = deps;
  const res = await fetch('/api/receipts/presign', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRF-Token': getCookie('csrf_token'),
    },
    credentials: 'include',
    body: JSON.stringify({ key, action: 'download' }),
  });
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
        const url = await getReceiptDownloadUrl(key, deps);
        window.open(url, '_blank');
      } catch (err) {
        alert('Failed to preview receipt');
      }
    });
  });
}
