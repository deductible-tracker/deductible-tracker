import {
  analyzeConfirmedReceipt,
  analyzeUploadedReceipt,
  confirmReceiptUpload,
  mapReceiptSuggestionToDonationDraft,
  uploadReceiptToStorage,
} from '../../../services/receipt-upload.js';

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

function buildDonationFormHtml(
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

async function bindDonationFormHandlers({ userId, charities, existingDonation }, deps) {
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
          const url = await getReceiptDownloadUrl(key, deps);
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
              deps.Sync.queueAction('receipts', { id: entry.server_id }, 'delete');
            }

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
        stage: canAttachImmediately ? 'attaching' : 'analyzing',
      });

      if (canAttachImmediately) {
        const confirmed = await confirmReceiptUpload(uploaded, existingDonation.id);
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
    if (e.target.closest('span')) return;
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
            'X-CSRF-Token': getCookie('csrf_token'),
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

  root.innerHTML = buildDonationFormHtml(
    {
      title: 'New Donation',
      desc: 'Add a donation and optionally attach receipts.',
      submitLabel: 'Save Donation',
      categoryPrefill: 'money',
    },
    deps
  );
  await bindDonationFormHandlers({ userId, charities, existingDonation: null }, deps);
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
  root.innerHTML = buildDonationFormHtml(
    {
      title: 'Edit Donation',
      desc: 'Update the details for this donation.',
      submitLabel: 'Save Changes',
      existing: { ...existing, _charityName: charityName },
    },
    deps
  );
  await bindDonationFormHandlers({ userId, charities, existingDonation: existing }, deps);
}
