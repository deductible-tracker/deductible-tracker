import db from './db.js';
import { getCurrentUserId } from './services/current-user.js';
import { getCookie } from './services/http.js';

function getToastHost() {
  let host = document.getElementById('toast-host');
  if (host) return host;

  host = document.createElement('div');
  host.id = 'toast-host';
  host.className =
    'fixed top-20 right-4 z-50 flex w-[22rem] max-w-[calc(100vw-2rem)] flex-col gap-2 pointer-events-none';
  document.body.appendChild(host);
  return host;
}

function showToast(message, type = 'info') {
  const host = getToastHost();
  const toast = document.createElement('div');

  const palette = {
    success: {
      badge: 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
      title: 'Success',
      titleColor: 'text-emerald-700 dark:text-emerald-300',
    },
    error: {
      badge: 'bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300',
      title: 'Upload failed',
      titleColor: 'text-rose-700 dark:text-rose-300',
    },
    info: {
      badge: 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300',
      title: 'Notice',
      titleColor: 'text-slate-700 dark:text-slate-300',
    },
  };

  const style = palette[type] || palette.info;

  toast.className =
    'pointer-events-auto rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 p-3 shadow-sm transition duration-300';

  const wrapper = document.createElement('div');
  wrapper.className = 'flex items-start gap-3';

  const iconSpan = document.createElement('span');
  iconSpan.className = `mt-0.5 inline-flex h-6 w-6 items-center justify-center rounded-full text-xs font-semibold ${style.badge}`;
  iconSpan.textContent = type === 'success' ? '✓' : type === 'error' ? '!' : 'i';
  wrapper.appendChild(iconSpan);

  const textContainer = document.createElement('div');
  textContainer.className = 'min-w-0 flex-1';

  const titleP = document.createElement('p');
  titleP.className = `text-sm font-semibold ${style.titleColor}`;
  titleP.textContent = style.title;
  textContainer.appendChild(titleP);

  const messageP = document.createElement('p');
  messageP.className = 'mt-0.5 text-sm text-slate-600 dark:text-slate-300 wrap-break-word';
  messageP.textContent = message;
  textContainer.appendChild(messageP);

  wrapper.appendChild(textContainer);

  const closeButton = document.createElement('button');
  closeButton.type = 'button';
  closeButton.className = 'toast-close text-slate-400 hover:text-slate-600 dark:text-slate-300';
  closeButton.setAttribute('aria-label', 'Dismiss');
  closeButton.textContent = '✕';
  wrapper.appendChild(closeButton);

  toast.appendChild(wrapper);

  const dismiss = () => {
    toast.classList.add('opacity-0', 'translate-x-2');
    setTimeout(() => toast.remove(), 220);
  };

  closeButton.addEventListener('click', dismiss);
  host.appendChild(toast);
  setTimeout(dismiss, 4200);
}

async function chooseDonationForUpload() {
  const userId = getCurrentUserId();
  if (!userId) return null;

  const donations = (await db.donations.where('user_id').equals(userId).toArray()).sort((a, b) =>
    String(b.date || '').localeCompare(String(a.date || ''))
  );
  if (donations.length === 0) {
    alert('Create a donation first, then upload a receipt for that donation.');
    return null;
  }
  if (donations.length === 1) {
    return donations[0].id;
  }

  const charities = await db.charities.where('user_id').equals(userId).toArray();
  const charityMap = new Map(charities.map((c) => [c.id, c.name || 'Unknown charity']));
  const shortlist = donations.slice(0, 20);
  const choices = shortlist
    .map(
      (d, index) =>
        `${index + 1}. ${d.date || 'Unknown date'} — ${charityMap.get(d.charity_id) || 'Unknown charity'}`
    )
    .join('\n');

  const raw = window.prompt(`Select donation number for this receipt:\n${choices}`);
  if (!raw) return null;
  const selectedIndex = parseInt(raw, 10) - 1;
  if (!Number.isInteger(selectedIndex) || selectedIndex < 0 || selectedIndex >= shortlist.length) {
    alert('Invalid selection. Upload cancelled.');
    return null;
  }
  return shortlist[selectedIndex].id;
}

// Simple uploader that requests a presigned URL and uploads the file
async function uploadFile(file, donationId) {
  try {
    const userId = getCurrentUserId();
    if (!userId) {
      throw new Error('Not authenticated');
    }
    const res = await fetch('/api/receipts/upload', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-CSRF-Token': getCookie('auth_token'),
      },
      credentials: 'include',
      body: JSON.stringify({ file_type: file.type }),
    });

    if (!res.ok) {
      throw new Error('Failed to get upload URL');
    }

    const data = await res.json();
    const uploadUrl = data.upload_url;
    const key = data.key;

    // Upload via PUT to presigned URL
    const putRes = await fetch(uploadUrl, {
      method: 'PUT',
      headers: {
        'Content-Type': file.type,
      },
      body: file,
    });

    if (!putRes.ok && putRes.status !== 200 && putRes.status !== 204) {
      throw new Error('Upload failed');
    }

    // Persist server-side record for metadata first
    const confirmRes = await fetch('/api/receipts/confirm', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-CSRF-Token': getCookie('auth_token'),
      },
      credentials: 'include',
      body: JSON.stringify({
        key,
        file_name: file.name,
        content_type: file.type,
        size: file.size,
        donation_id: donationId,
      }),
    });

    if (!confirmRes.ok) {
      throw new Error('Failed to confirm receipt');
    }

    const body = await confirmRes.json();
    const id = crypto.randomUUID();
    const meta = {
      id,
      key,
      file_name: file.name,
      content_type: file.type,
      size: file.size,
      uploaded_at: new Date().toISOString(),
      donation_id: donationId,
      status: 'uploaded',
      server_id: body && body.id ? body.id : null,
    };

    await db.receipts.put(meta);
    // Trigger OCR asynchronously on the server for the confirmed receipt
    try {
      if (body && body.id) {
        fetch('/api/receipts/ocr', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': getCookie('auth_token'),
          },
          credentials: 'include',
          body: JSON.stringify({ id: body.id }),
        }).catch((e) => console.warn('OCR request failed', e));
      }
    } catch (e) {
      /* ignore */
    }
    return meta;
  } catch (err) {
    console.error('Upload error', err);
    throw err;
  }
}

function createFileInputAndUpload() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = 'image/*,application/pdf';
  input.style.display = 'none';

  input.addEventListener('change', async (e) => {
    const f = e.target.files && e.target.files[0];
    if (!f) return;
    try {
      const donationId = await chooseDonationForUpload();
      if (!donationId) {
        return;
      }
      const meta = await uploadFile(f, donationId);
      showToast(`Uploaded ${meta.file_name}`, 'success');
    } catch (err) {
      const message = err && err.message ? err.message : 'Please try again.';
      showToast(message, 'error');
    } finally {
      document.body.removeChild(input);
    }
  });

  document.body.appendChild(input);
  input.click();
}

// Attach handler when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    const btn = document.getElementById('btn-upload-receipt');
    if (btn) btn.addEventListener('click', createFileInputAndUpload);
  });
} else {
  const btn = document.getElementById('btn-upload-receipt');
  if (btn) btn.addEventListener('click', createFileInputAndUpload);
}
