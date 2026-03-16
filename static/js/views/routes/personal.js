function normalizeFilingStatus(status) {
  const normalized = String(status || 'single').toLowerCase();
  if (
    normalized === 'married_joint' ||
    normalized === 'married_separate' ||
    normalized === 'head_household' ||
    normalized === 'single'
  ) {
    return normalized;
  }
  return 'single';
}

async function fetchMarginalRateDataRoute(filingStatus, agi, deps) {
  const params = new URLSearchParams();
  params.set('filing_status', normalizeFilingStatus(filingStatus));
  if (Number.isFinite(agi) && agi >= 0) params.set('agi', String(agi));
  const { res, data } = await deps.apiJson(`/api/tax/marginal-rate?${params.toString()}`);
  if (!res.ok || !data) throw new Error('Failed to fetch marginal tax rate');
  return {
    brackets: Array.isArray(data.brackets) ? data.brackets : [],
    selectedRate: Number.isFinite(Number(data.selected_rate)) ? Number(data.selected_rate) : null,
  };
}

function renderRateOptionsRoute(brackets, currentRate) {
  if (!brackets || brackets.length === 0) {
    const defaults = [0.1, 0.12, 0.22, 0.24, 0.32, 0.35, 0.37];
    return defaults
      .map((r) => {
        const label = `${Math.round(r * 100)}%`;
        const sel = Math.abs(r - Number(currentRate)) < 0.001 ? ' selected' : '';
        return `<option value="${r.toFixed(2)}"${sel}>${label}</option>`;
      })
      .join('');
  }
  return brackets
    .map((b) => {
      const pct = `${Math.round(b.rate * 100)}%`;
      const range =
        b.max === null
          ? `$${Number(b.min).toLocaleString()}+`
          : `$${Number(b.min).toLocaleString()} – $${Number(b.max).toLocaleString()}`;
      const sel = Math.abs(b.rate - Number(currentRate)) < 0.001 ? ' selected' : '';
      return `<option value="${b.rate.toFixed(2)}"${sel}>${pct} (${range})</option>`;
    })
    .join('');
}

export async function renderPersonalInfoRoute(deps) {
  const root = document.getElementById('route-content') || document.getElementById('app');
  let profile = {
    name: '',
    email: '',
    filing_status: 'single',
    agi: '',
    marginal_tax_rate: '0.22',
    itemize_deductions: false,
  };

  const cached = deps.getCurrentUser();
  if (cached && cached.id) {
    profile = {
      ...profile,
      name: cached.name || '',
      email: cached.email || '',
      filing_status: normalizeFilingStatus(cached.filing_status),
      agi: cached.agi ?? '',
      marginal_tax_rate: cached.marginal_tax_rate ?? '0.22',
      itemize_deductions: !!cached.itemize_deductions,
    };
  }

  if (navigator.onLine) {
    try {
      const { res, data } = await deps.apiJson('/api/me');
      if (res.ok && data) {
        profile = {
          name: data.name || '',
          email: data.email || '',
          filing_status: normalizeFilingStatus(data.filing_status),
          agi: data.agi ?? '',
          marginal_tax_rate: data.marginal_tax_rate ?? '0.22',
          itemize_deductions: !!data.itemize_deductions,
        };
      }
    } catch (e) {
      console.warn('Failed to load profile from server', e);
    }
  }

  let initialRateData = { brackets: [], selectedRate: null };
  if (navigator.onLine) {
    try {
      initialRateData = await fetchMarginalRateDataRoute(
        profile.filing_status,
        Number(profile.agi),
        deps
      );
      if (initialRateData.selectedRate !== null)
        profile.marginal_tax_rate = initialRateData.selectedRate;
    } catch (e) {
      console.warn('Failed to fetch initial marginal tax rate', e);
    }
  }

  root.innerHTML = `
        <div class="mx-auto max-w-3xl space-y-4">
            <div>
                <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Personal Info</h1>
                <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Maintain your profile and tax inputs for more accurate IRS-based savings estimates.</p>
            </div>
            <div class="dt-panel p-6">
                <form id="personal-form" class="space-y-4">
                    <div>
                        <label class="dt-label">Full name</label>
                        <input id="profile-name" type="text" value="${deps.escapeHtml(profile.name)}" class="dt-input" />
                    </div>
                    <div>
                        <label class="dt-label">Email</label>
                        <input id="profile-email" type="email" value="${deps.escapeHtml(profile.email)}" class="dt-input" />
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Filing Status</label>
                            <select id="profile-filing-status" class="dt-input">
                                <option value="single" ${profile.filing_status === 'single' ? 'selected' : ''}>Single</option>
                                <option value="married_joint" ${profile.filing_status === 'married_joint' ? 'selected' : ''}>Married filing jointly</option>
                                <option value="married_separate" ${profile.filing_status === 'married_separate' ? 'selected' : ''}>Married filing separately</option>
                                <option value="head_household" ${profile.filing_status === 'head_household' ? 'selected' : ''}>Head of household</option>
                            </select>
                        </div>
                        <div>
                            <label class="dt-label">Adjusted Gross Income (AGI)</label>
                            <input id="profile-agi" type="number" min="0" step="0.01" value="${deps.escapeHtml(String(profile.agi))}" class="dt-input" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Marginal Tax Rate</label>
                            <select id="profile-marginal-rate" class="dt-input">
                                ${renderRateOptionsRoute(initialRateData.brackets, profile.marginal_tax_rate)}
                            </select>
                            ${!navigator.onLine ? '<p class="mt-1 text-xs text-amber-600 dark:text-amber-400">Offline — showing default brackets. AGI-based selection available when connected.</p>' : ''}
                        </div>
                        <div class="flex items-end">
                            <label class="inline-flex items-center gap-2 pb-2 text-sm text-slate-700 dark:text-slate-300">
                                <input id="profile-itemize" type="checkbox" class="h-4 w-4 rounded border-slate-300" ${profile.itemize_deductions ? 'checked' : ''} />
                                I itemize deductions on Schedule A
                            </label>
                        </div>
                    </div>
                    <p class="text-xs text-slate-500 dark:text-slate-400">Marginal tax rate uses IRS 2025 federal income tax brackets based on filing status and AGI.</p>
                    <p class="text-xs text-slate-500 dark:text-slate-400">2026 rule note: non-itemizers may deduct up to $1,000 cash contributions ($2,000 married filing jointly).</p>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">Save</button>
                    </div>
                    </form>
                    </div>

                    <div class="dt-panel p-6">
                    <h2 class="text-lg font-medium text-slate-900 dark:text-slate-100">Data Management</h2>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Backup your data to a restorable ZIP file or restore from a previous backup.</p>
                    <div class="mt-4 flex flex-wrap gap-4">
                    <button id="backup-btn" class="dt-btn-secondary flex items-center gap-2">
                        <i data-lucide="download" class="h-4 w-4"></i>
                        Backup My Data
                    </button>
                    <button id="restore-btn" class="dt-btn-secondary flex items-center gap-2">
                        <i data-lucide="upload" class="h-4 w-4"></i>
                        Restore Data
                    </button>
                    <input id="restore-input" type="file" accept=".zip" class="hidden" />
                    </div>
                    </div>

                    <div class="dt-panel p-6 border-red-200 dark:border-red-900/30">
                    <h2 class="text-lg font-medium text-red-600 dark:text-red-400">Danger Zone</h2>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Once you delete your account, there is no going back. All your donations, receipts, and personal data will be permanently removed.</p>
                    <div class="mt-4">
                    <button id="delete-account-btn" class="inline-flex items-center justify-center rounded-md bg-red-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-red-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-red-600">
                        Delete My Account
                    </button>
                    </div>
                    </div>
                    </div>

    `;

  const filingStatusEl = document.getElementById('profile-filing-status');
  const agiEl = document.getElementById('profile-agi');
  const marginalRateEl = document.getElementById('profile-marginal-rate');
  let rateRequestCounter = 0;

  if (window.lucide) {
    window.lucide.createIcons();
  }

  document.getElementById('backup-btn')?.addEventListener('click', async () => {
    try {
      const res = await fetch('/api/me/export', { credentials: 'include' });
      if (res.ok) {
        const blob = await res.blob();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `deductible-tracker-backup-${new Date().toISOString().split('T')[0]}.zip`;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        a.remove();
      } else {
        alert('Failed to generate backup');
      }
    } catch (e) {
      console.error('Backup error', e);
      alert('An error occurred during backup');
    }
  });

  const restoreInput = document.getElementById('restore-input');
  document.getElementById('restore-btn')?.addEventListener('click', () => {
    restoreInput?.click();
  });

  restoreInput?.addEventListener('change', async (e) => {
    const file = e.target.files?.[0];
    if (!file) return;

    if (!confirm('This will overwrite your profile and merge imported data. Continue?')) {
      e.target.value = '';
      return;
    }

    const formData = new FormData();
    formData.append('file', file);

    try {
      const csrfToken = deps.getCookie('auth_token');
      const res = await fetch('/api/me/import', {
        method: 'POST',
        credentials: 'include',
        headers: { 'X-CSRF-Token': csrfToken || '' },
        body: formData,
      });

      if (res.ok) {
        alert('Data restored successfully. The page will now reload.');
        await deps.db.delete();
        await deps.db.open();
        window.location.reload();
      } else {
        const txt = await res.text();
        alert(`Restore failed: ${txt}`);
      }
    } catch (e) {
      console.error('Restore error', e);
      alert('An error occurred during restore');
    } finally {
      e.target.value = '';
    }
  });

  document.getElementById('delete-account-btn')?.addEventListener('click', async () => {
    if (!confirm('Are you absolutely sure you want to delete your account? This cannot be undone.')) {
      return;
    }

    if (
      !confirm(
        'LAST WARNING: All your data, including receipts in cloud storage, will be permanently deleted.'
      )
    ) {
      return;
    }

    try {
      const { res } = await deps.apiJson('/api/me', { method: 'DELETE' });
      if (res.ok) {
        await deps.db.delete();
        await deps.db.open();
        alert('Your account has been deleted.');
        window.location.href = '/auth/logout';
      } else {
        alert('Failed to delete account. Please try again or contact support.');
      }
    } catch (e) {
      console.error('Delete account error', e);
      alert('An error occurred. Please try again.');
    }
  });

  const syncMarginalRateFromServer = async () => {
    if (!navigator.onLine) return;
    const requestId = ++rateRequestCounter;
    const filingStatus = normalizeFilingStatus(filingStatusEl.value);
    const agi = parseFloat(agiEl.value || '');
    try {
      const { brackets, selectedRate } = await fetchMarginalRateDataRoute(filingStatus, agi, deps);
      if (requestId !== rateRequestCounter) return;
      const currentVal = marginalRateEl.value;
      marginalRateEl.innerHTML = renderRateOptionsRoute(brackets, currentVal);
      if (selectedRate !== null) marginalRateEl.value = selectedRate.toFixed(2);
    } catch (e) {
      console.warn('Failed to sync marginal tax rate', e);
    }
  };

  filingStatusEl?.addEventListener('change', syncMarginalRateFromServer);
  agiEl?.addEventListener('change', syncMarginalRateFromServer);

  document.getElementById('personal-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const updated = {
      name: document.getElementById('profile-name').value.trim(),
      email: document.getElementById('profile-email').value.trim(),
      filing_status: filingStatusEl.value,
      agi: parseFloat(agiEl.value || ''),
      marginal_tax_rate: parseFloat(marginalRateEl.value || ''),
      itemize_deductions: document.getElementById('profile-itemize').checked,
    };
    if (!Number.isFinite(updated.agi)) updated.agi = null;
    if (!Number.isFinite(updated.marginal_tax_rate)) updated.marginal_tax_rate = null;

    deps.setCurrentUser({ ...(deps.getCurrentUser() || {}), ...updated });

    try {
      const { res, data } = await deps.apiJson('/api/me', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updated),
      });
      if (!res.ok) throw new Error(typeof data === 'string' ? data : 'Failed to save profile');
      if (data && data.id) deps.setCurrentUser(data);
      alert('Saved');
      await deps.updateTotals();
    } catch (err) {
      console.warn('Profile save failed', err);
      await deps.Sync.queueProfileUpdate(deps.getCurrentUserId(), updated);
      alert('Saved locally. Will sync when online.');
      await deps.updateTotals();
    }
  });
}
