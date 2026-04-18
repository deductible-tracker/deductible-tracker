import { apiJson } from './http.js';
import {
  encryptCharityPayload,
  encryptDonationPayload,
  decryptCharityItems,
} from './encrypt-transport.js';

export async function createOrGetCharityOnServer(nameOrPayload, ein) {
  const payload =
    typeof nameOrPayload === 'object' && nameOrPayload !== null
      ? nameOrPayload
      : { name: nameOrPayload, ein };

  const finalPayload = await encryptCharityPayload(
    payload,
    `Encrypted Charity (${payload.id || 'new'})`
  );

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
  const finalPayload = await encryptCharityPayload(
    payload,
    `Encrypted Charity (${charityId})`
  );

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
  return decryptCharityItems(charities);
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
  const finalPayload = await encryptDonationPayload(payload);

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
  const finalPayload = await encryptDonationPayload(payload);

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
