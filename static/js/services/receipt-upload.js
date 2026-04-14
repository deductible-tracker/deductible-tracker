import { apiJson } from './http.js';

export function isImageReceipt(contentType) {
  return contentType === 'image/jpeg' || contentType === 'image/png';
}

export async function requestReceiptUpload(fileType) {
  const { res, data } = await apiJson('/api/receipts/upload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_type: fileType }),
  });

  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to request upload URL');
  }
  return data;
}

export async function uploadReceiptBinary(uploadUrl, file) {
  const res = await fetch(uploadUrl, {
    method: 'PUT',
    headers: { 'Content-Type': file.type || 'application/octet-stream' },
    body: file,
  });

  if (!res.ok && res.status !== 200 && res.status !== 204) {
    throw new Error('Receipt upload failed');
  }
}

export async function uploadReceiptToStorage(file) {
  const uploadData = await requestReceiptUpload(file.type);
  await uploadReceiptBinary(uploadData.upload_url, file);
  return {
    key: uploadData.key,
    file_name: file.name,
    content_type: file.type,
    size: file.size,
  };
}

export async function confirmReceiptUpload(uploadedReceipt, donationId) {
  const { res, data } = await apiJson('/api/receipts/confirm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      key: uploadedReceipt.key,
      file_name: uploadedReceipt.file_name,
      content_type: uploadedReceipt.content_type,
      size: uploadedReceipt.size,
      donation_id: donationId,
    }),
  });

  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to confirm receipt');
  }

  return {
    ...uploadedReceipt,
    donation_id: donationId,
    id: data && data.id ? data.id : null,
  };
}

export function normalizeReceiptAnalysis(data) {
  if (!data || typeof data !== 'object') return null;

  const suggestion =
    data.suggestion && typeof data.suggestion === 'object' ? data.suggestion : null;
  const amount = Number(data.ocr_amount_usd);
  const suggestionAmount = Number(suggestion && suggestion.amount_usd);

  return {
    status: typeof data.status === 'string' ? data.status : 'unknown',
    warning: typeof data.warning === 'string' ? data.warning : null,
    ocrText: typeof data.ocr_text === 'string' ? data.ocr_text : null,
    ocrDate: typeof data.ocr_date === 'string' ? data.ocr_date : null,
    ocrAmountUsd: Number.isFinite(amount) ? amount : null,
    suggestion: suggestion
      ? {
          dateOfDonation:
            typeof suggestion.date_of_donation === 'string' ? suggestion.date_of_donation : null,
          organizationName:
            typeof suggestion.organization_name === 'string'
              ? suggestion.organization_name.trim() || null
              : null,
          donationType:
            suggestion.donation_type === 'item' || suggestion.donation_type === 'money'
              ? suggestion.donation_type
              : null,
          itemName:
            typeof suggestion.item_name === 'string' ? suggestion.item_name.trim() || null : null,
          amountUsd: Number.isFinite(suggestionAmount) ? suggestionAmount : null,
        }
      : null,
  };
}

export async function analyzeReceipt(payload) {
  const { res, data } = await apiJson('/api/receipts/ocr', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to analyze receipt');
  }

  return normalizeReceiptAnalysis(data);
}

export async function analyzeUploadedReceipt(uploadedReceipt) {
  return analyzeReceipt({
    key: uploadedReceipt.key,
    content_type: uploadedReceipt.content_type,
    size: uploadedReceipt.size,
  });
}

export async function analyzeConfirmedReceipt(receiptId) {
  return analyzeReceipt({ id: receiptId });
}

export async function attachReceiptFileToDonation(file, donationId) {
  const uploaded = await uploadReceiptToStorage(file);
  const confirmed = await confirmReceiptUpload(uploaded, donationId);
  const analysis = confirmed.id ? await analyzeConfirmedReceipt(confirmed.id) : null;
  return { uploaded, confirmed, analysis };
}

export function mapReceiptSuggestionToDonationDraft(analysis) {
  const suggestion = analysis && analysis.suggestion;
  if (!suggestion || !suggestion.donationType) return null;

  return {
    date: suggestion.dateOfDonation || null,
    charityName: suggestion.organizationName || null,
    category: suggestion.donationType === 'item' ? 'items' : 'money',
    itemName: suggestion.donationType === 'item' ? suggestion.itemName || null : null,
    amount:
      suggestion.donationType === 'money' && Number.isFinite(suggestion.amountUsd)
        ? suggestion.amountUsd
        : null,
  };
}
