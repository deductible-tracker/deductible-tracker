import {
  encryptCharityPayload,
  encryptDonationPayload,
  decryptCharityItems,
} from "./encrypt-transport";

export function getCookie(name: string): string | null {
  if (typeof document === "undefined") return null;
  const value = `; ${document.cookie}`;
  const parts = value.split(`; ${name}=`);
  if (parts.length >= 2) return parts.pop()?.split(";").shift() || null;
  return null;
}

export async function apiJson(path: string, options: any = {}): Promise<{ res: Response; data: any }> {
  const method = (options.method || "GET").toUpperCase();
  const headers = { ...options.headers };

  if (["POST", "PUT", "DELETE", "PATCH"].includes(method)) {
    const token = getCookie("csrf_token");
    if (token) {
      headers["X-CSRF-Token"] = token;
    }
  }

  const res = await fetch(path, {
    credentials: "include",
    ...options,
    headers,
  });

  let data = null;
  const contentType = res.headers.get("content-type") || "";
  if (contentType.includes("application/json")) {
    try {
      data = await res.json();
    } catch (e) {
      /* ignore */
    }
  } else {
    try {
      data = await res.text();
    } catch (e) {
      /* ignore */
    }
  }

  return { res, data };
}

export async function createOrGetCharityOnServer(nameOrPayload: any, ein?: string) {
  const payload =
    typeof nameOrPayload === "object" && nameOrPayload !== null
      ? nameOrPayload
      : { name: nameOrPayload, ein };

  const finalPayload = await encryptCharityPayload(
    payload,
    `Encrypted Charity (${payload.id || "new"})`
  );

  const { res, data } = await apiJson("/api/charities", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to create charity");
  }
  return data;
}

export async function lookupCharityByEinOnServer(ein: string) {
  const normalizedEin = (ein || "").replace(/\D/g, "");
  if (!normalizedEin) return null;
  const { res, data } = await apiJson(`/api/charities/lookup/${encodeURIComponent(normalizedEin)}`);
  if (!res.ok) return null;
  return data; // returns the lookup charity response from ProPublica directly
}

export async function updateCharityOnServer(charityId: string, payload: any) {
  const finalPayload = await encryptCharityPayload(
    payload,
    `Encrypted Charity (${charityId})`
  );

  const { res, data } = await apiJson(`/api/charities/${encodeURIComponent(charityId)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to update charity");
  }
  return data;
}

export async function fetchCharitiesFromServer() {
  const { res, data } = await apiJson("/api/charities");
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to fetch charities");
  }
  const charities = data && data.charities ? data.charities : [];
  return decryptCharityItems(charities);
}

export async function deleteCharityOnServer(charityId: string) {
  const { res, data } = await apiJson(`/api/charities/${encodeURIComponent(charityId)}`, {
    method: "DELETE",
  });
  if (res.status === 409 || res.status === 400) {
    throw new Error("Charity has donations and cannot be deleted");
  }
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to delete charity");
  }
}

export async function createDonationOnServer(payload: any) {
  const finalPayload = await encryptDonationPayload(payload);

  const { res, data } = await apiJson("/api/donations", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to create donation");
  }
  return data;
}

export async function updateDonationOnServer(donationId: string, payload: any) {
  const finalPayload = await encryptDonationPayload(payload);

  const { res, data } = await apiJson(`/api/donations/${encodeURIComponent(donationId)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(finalPayload),
  });
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to update donation");
  }
  return data;
}

export async function deleteDonationOnServer(donationId: string) {
  const { res, data } = await apiJson(`/api/donations/${encodeURIComponent(donationId)}`, {
    method: "DELETE",
  });
  if (!res.ok) {
    throw new Error(typeof data === "string" ? data : "Failed to delete donation");
  }
}
