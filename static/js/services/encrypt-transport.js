/**
 * Encrypt/decrypt helpers for API transport.
 *
 * Wraps the vault-key encryption pattern used across api-client, sync,
 * and receipt-upload to avoid duplicating the same field-nullification
 * logic in every call-site.
 */

import { encryptData, decryptData, ensureVaultKey } from './crypto.js';
import { getCurrentUserId } from './current-user.js';

const CHARITY_SENSITIVE_FIELDS = ['name', 'ein', 'street', 'city', 'state', 'zip'];
const DONATION_SENSITIVE_FIELDS = ['date', 'category', 'amount', 'notes'];

/**
 * Encrypt specified fields in a payload and nullify the originals.
 * Returns the original payload unchanged when vaultKey is null.
 *
 * @param {CryptoKey|null} vaultKey
 * @param {object}  payload
 * @param {string[]} fields - field names to encrypt
 * @param {string}  [placeholderLabel] - optional placeholder for the `name` field
 * @returns {Promise<object>}
 */
export async function encryptPayloadFields(vaultKey, payload, fields, placeholderLabel) {
  if (!vaultKey) return payload;

  const sensitive = {};
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

  if (placeholderLabel && 'name' in result) {
    result.name = placeholderLabel;
  }

  return result;
}

/**
 * Decrypt `encrypted_payload` on items that have `is_encrypted` set.
 * Mutates items in-place and returns the array.
 *
 * @param {CryptoKey|null} vaultKey
 * @param {object[]} items
 * @returns {Promise<object[]>}
 */
export async function decryptPayloadItems(vaultKey, items) {
  if (!vaultKey) return items;

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.is_encrypted && item.encrypted_payload) {
      try {
        const decrypted = await decryptData(vaultKey, item.encrypted_payload);
        items[i] = { ...item, ...decrypted };
      } catch (e) {
        console.error('Failed to decrypt item', item.id, e);
      }
    }
  }

  return items;
}

/**
 * Convenience: get vault key and encrypt a charity payload.
 */
export async function encryptCharityPayload(payload, label) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  return encryptPayloadFields(vaultKey, payload, CHARITY_SENSITIVE_FIELDS, label);
}

/**
 * Convenience: get vault key and encrypt a donation payload.
 */
export async function encryptDonationPayload(payload) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  return encryptPayloadFields(vaultKey, payload, DONATION_SENSITIVE_FIELDS);
}

/**
 * Convenience: get vault key and decrypt an array of charity items.
 */
export async function decryptCharityItems(charities) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  return decryptPayloadItems(vaultKey, charities);
}

export { CHARITY_SENSITIVE_FIELDS, DONATION_SENSITIVE_FIELDS };
