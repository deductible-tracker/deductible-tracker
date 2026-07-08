import { encryptData, decryptData, ensureVaultKey } from "./crypto";
import { getCurrentUserId } from "./currentUser";

export const CHARITY_SENSITIVE_FIELDS = ["name", "ein", "street", "city", "state", "zip"];
export const DONATION_SENSITIVE_FIELDS = ["date", "category", "amount", "notes"];

export async function encryptPayloadFields(
  vaultKey: CryptoKey | null,
  payload: any,
  fields: string[],
  placeholderLabel?: string
): Promise<any> {
  if (!vaultKey) return payload;

  const sensitive: any = {};
  for (const f of fields) {
    sensitive[f] = payload[f];
  }

  const encrypted = await encryptData(vaultKey, sensitive);
  const result = {
    ...payload,
    is_encrypted: true,
    encrypted_payload: encrypted,
  };

  for (const f of fields) {
    result[f] = null;
  }

  if (placeholderLabel && "name" in result) {
    result.name = placeholderLabel;
  }

  return result;
}

export async function decryptPayloadItems(vaultKey: CryptoKey | null, items: any[]): Promise<any[]> {
  if (!vaultKey) return items;

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.is_encrypted && item.encrypted_payload) {
      try {
        const decrypted = await decryptData(vaultKey, item.encrypted_payload);
        items[i] = { ...item, ...decrypted };
      } catch (e) {
        console.error("Failed to decrypt item", item.id, e);
      }
    }
  }

  return items;
}

export async function encryptCharityPayload(payload: any, label?: string): Promise<any> {
  const userId = getCurrentUserId();
  if (!userId) return payload;
  const vaultKey = await ensureVaultKey(userId);
  return encryptPayloadFields(vaultKey, payload, CHARITY_SENSITIVE_FIELDS, label);
}

export async function encryptDonationPayload(payload: any): Promise<any> {
  const userId = getCurrentUserId();
  if (!userId) return payload;
  const vaultKey = await ensureVaultKey(userId);
  return encryptPayloadFields(vaultKey, payload, DONATION_SENSITIVE_FIELDS);
}

export async function decryptCharityItems(charities: any[]): Promise<any[]> {
  const userId = getCurrentUserId();
  if (!userId) return charities;
  const vaultKey = await ensureVaultKey(userId);
  return decryptPayloadItems(vaultKey, charities);
}
