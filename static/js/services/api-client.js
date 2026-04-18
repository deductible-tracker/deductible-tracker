import { apiJson } from './http.js';
import { ensureVaultKey, encryptData, decryptData } from './crypto.js';
import { getCurrentUserId } from './current-user.js';

export async function createOrGetCharityOnServer(nameOrPayload, ein) {
  const payload =
    typeof nameOrPayload === 'object' && nameOrPayload !== null
      ? nameOrPayload
      : { name: nameOrPayload, ein };

  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  let finalPayload = payload;

  if (vaultKey) {
    const sensitive = {
      name: payload.name,
      ein: payload.ein,
      street: payload.street,
      city: payload.city,
      state: payload.state,
      zip: payload.zip,
    };
    finalPayload = {
      ...payload,
      is_encrypted: true,
      encrypted_payload: await encryptData(vaultKey, sensitive),
      // Nullify PII for transport
      name: `Encrypted Charity (${payload.id || 'new'})`,
      ein: null,
      street: null,
      city: null,
      state: null,
      zip: null,
    };
  }

  const { res, data } = await apiJson('/api/charities', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to create charity');
  }
  return data;
}

export async function lookupCharityByEinOnServer(ein) {
  const normalizedEin = (ein || '').replace(/\D/g, '');
  if (!normalizedEin) return null;
  const { res, data } = await apiJson(`/api/charities/lookup/${encodeURIComponent(normalizedEin)}`);
  if (!res.ok) return null;
  return data && data.charity ? data.charity : null;
}

export async function updateCharityOnServer(charityId, payload) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  let finalPayload = payload;

  if (vaultKey) {
    const sensitive = {
      name: payload.name,
      ein: payload.ein,
      street: payload.street,
      city: payload.city,
      state: payload.state,
      zip: payload.zip,
    };
    finalPayload = {
      ...payload,
      is_encrypted: true,
      encrypted_payload: await encryptData(vaultKey, sensitive),
      // Nullify PII for transport
      name: `Encrypted Charity (${charityId})`,
      ein: null,
      street: null,
      city: null,
      state: null,
      zip: null,
    };
  }

  const { res, data } = await apiJson(`/api/charities/${encodeURIComponent(charityId)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to update charity');
  }
  return data;
}

export async function fetchCharitiesFromServer() {
  const { res, data } = await apiJson('/api/charities');
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to fetch charities');
  }
  const charities = data && data.charities ? data.charities : [];
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);

  if (vaultKey) {
    for (let i = 0; i < charities.length; i++) {
      const c = charities[i];
      if (c.is_encrypted && c.encrypted_payload) {
        try {
          const decrypted = await decryptData(vaultKey, c.encrypted_payload);
          charities[i] = { ...c, ...decrypted };
        } catch (e) {
          console.error('Failed to decrypt charity', c.id, e);
        }
      }
    }
  }

  return charities;
}

export async function deleteCharityOnServer(charityId) {
  const { res, data } = await apiJson(`/api/charities/${encodeURIComponent(charityId)}`, {
    method: 'DELETE',
  });
  if (res.status === 409) {
    throw new Error('Charity has donations and cannot be deleted');
  }
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to delete charity');
  }
}

export async function createDonationOnServer(payload) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  let finalPayload = payload;

  if (vaultKey) {
    const sensitive = {
      date: payload.date,
      category: payload.category,
      amount: payload.amount,
      notes: payload.notes,
    };
    finalPayload = {
      ...payload,
      is_encrypted: true,
      encrypted_payload: await encryptData(vaultKey, sensitive),
      // Nullify plaintext for transport
      date: null,
      category: null,
      amount: null,
      notes: null,
    };
  }

  const { res, data } = await apiJson('/api/donations', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to create donation');
  }
  return data;
}

export async function updateDonationOnServer(donationId, payload) {
  const userId = getCurrentUserId();
  const vaultKey = await ensureVaultKey(userId);
  let finalPayload = payload;

  if (vaultKey) {
    const sensitive = {
      date: payload.date,
      category: payload.category,
      amount: payload.amount,
      notes: payload.notes,
    };
    finalPayload = {
      ...payload,
      is_encrypted: true,
      encrypted_payload: await encryptData(vaultKey, sensitive),
      // Nullify plaintext for transport
      date: null,
      category: null,
      amount: null,
      notes: null,
    };
  }

  const { res, data } = await apiJson(`/api/donations/${encodeURIComponent(donationId)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to update donation');
  }
  return data;
}

export async function deleteDonationOnServer(donationId) {
  const { res, data } = await apiJson(`/api/donations/${encodeURIComponent(donationId)}`, {
    method: 'DELETE',
  });
  if (!res.ok) {
    throw new Error(typeof data === 'string' ? data : 'Failed to delete donation');
  }
}
