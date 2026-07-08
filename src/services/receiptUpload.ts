import { apiJson } from "./apiClient";
import { ensureVaultKey, encryptData, encryptBinaryData } from "./crypto";
import { getCurrentUserId } from "./currentUser";

const MAX_RECEIPT_SIZE_BYTES = 10 * 1024 * 1024;

export function isImageReceipt(contentType: string): boolean {
  return contentType === "image/jpeg" || contentType === "image/png" || contentType === "image/webp";
}

export async function requestReceiptUpload(fileType: string) {
  const { res, data } = await apiJson("/api/receipts/upload", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ file_type: fileType }),
  });

  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to request upload URL");
  }
  return data;
}

export async function uploadReceiptBinary(uploadUrl: string, file: Blob) {
  const res = await fetch(uploadUrl, {
    method: "PUT",
    headers: { "Content-Type": file.type || "application/octet-stream" },
    body: file,
  });

  if (!res.ok && res.status !== 200 && res.status !== 204) {
    throw new Error("Receipt upload failed");
  }
}

export async function uploadReceiptToStorage(file: File) {
  const userId = getCurrentUserId();
  if (!userId) throw new Error("User not logged in");
  const vaultKey = await ensureVaultKey(userId);
  
  let uploadFile: Blob = file;
  let isEncrypted = false;
  let encryptedPayload: string | null = null;

  if (vaultKey) {
    const buffer = await file.arrayBuffer();
    const base64Encrypted = await encryptBinaryData(vaultKey, buffer);
    const binaryEncrypted = Uint8Array.from(atob(base64Encrypted), (c) => c.charCodeAt(0));
    uploadFile = new Blob([binaryEncrypted], { type: "application/octet-stream" });
    isEncrypted = true;
    
    // Encrypt filename as part of metadata
    encryptedPayload = await encryptData(vaultKey, { file_name: file.name });
  }

  const uploadData = await requestReceiptUpload(uploadFile.type || "application/octet-stream");
  await uploadReceiptBinary(uploadData.upload_url, uploadFile);
  
  return {
    key: uploadData.key,
    file_name: isEncrypted ? null : file.name,
    content_type: file.type,
    size: uploadFile.size,
    is_encrypted: isEncrypted,
    encrypted_payload: encryptedPayload,
  };
}

export async function confirmReceiptUpload(uploadedReceipt: any, donationId: string) {
  const { res, data } = await apiJson("/api/receipts/confirm", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      key: uploadedReceipt.key,
      file_name: uploadedReceipt.file_name,
      content_type: uploadedReceipt.content_type,
      size: uploadedReceipt.size,
      donation_id: donationId,
      is_encrypted: uploadedReceipt.is_encrypted,
      encrypted_payload: uploadedReceipt.encrypted_payload,
    }),
  });

  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to confirm receipt");
  }

  return {
    ...uploadedReceipt,
    donation_id: donationId,
    id: data && data.id ? data.id : null,
  };
}

export function normalizeReceiptAnalysis(data: any) {
  if (!data || typeof data !== "object") return null;

  const suggestion =
    data.suggestion && typeof data.suggestion === "object" ? data.suggestion : null;
  const amount = Number(data.ocr_amount_usd);
  const suggestionAmount = Number(suggestion && suggestion.amount_usd);

  return {
    status: typeof data.status === "string" ? data.status : "unknown",
    warning: typeof data.warning === "string" ? data.warning : null,
    ocrText: typeof data.ocr_text === "string" ? data.ocr_text : null,
    ocrDate: typeof data.ocr_date === "string" ? data.ocr_date : null,
    ocrAmountUsd: Number.isFinite(amount) ? amount : null,
    suggestion: suggestion
      ? {
          dateOfDonation:
            typeof suggestion.date_of_donation === "string" ? suggestion.date_of_donation : null,
          organizationName:
            typeof suggestion.organization_name === "string"
              ? suggestion.organization_name.trim() || null
              : null,
          donationType:
            suggestion.donation_type === "item" || suggestion.donation_type === "money"
              ? suggestion.donation_type
              : null,
          itemName:
            typeof suggestion.item_name === "string" ? suggestion.item_name.trim() || null : null,
          amountUsd: Number.isFinite(suggestionAmount) ? suggestionAmount : null,
        }
      : null,
  };
}

export async function analyzeReceipt(payload: any) {
  const userId = getCurrentUserId();
  if (!userId) throw new Error("User not logged in");
  const vaultKey = await ensureVaultKey(userId);
  const headers: Record<string, string> = { "Content-Type": "application/json" };

  if (vaultKey) {
    const rawKey = await crypto.subtle.exportKey("raw", vaultKey);
    headers["X-Vault-Key"] = btoa(String.fromCharCode(...new Uint8Array(rawKey)));
  }

  const { res, data } = await apiJson("/api/receipts/ocr", {
    method: "POST",
    headers,
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to analyze receipt");
  }

  return normalizeReceiptAnalysis(data);
}

export async function analyzeUploadedReceipt(uploadedReceipt: any) {
  return analyzeReceipt({
    key: uploadedReceipt.key,
    content_type: uploadedReceipt.content_type,
    size: uploadedReceipt.size,
  });
}

export async function analyzeConfirmedReceipt(receiptId: string) {
  return analyzeReceipt({ id: receiptId });
}

export async function attachReceiptFileToDonation(file: File, donationId: string) {
  const uploaded = await uploadReceiptToStorage(file);
  const confirmed = await confirmReceiptUpload(uploaded, donationId);
  const analysis = confirmed.id ? await analyzeConfirmedReceipt(confirmed.id) : null;
  return { uploaded, confirmed, analysis };
}
