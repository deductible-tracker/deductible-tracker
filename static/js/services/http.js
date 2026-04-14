export async function apiJson(path, options = {}) {
  const method = (options.method || 'GET').toUpperCase();
  const headers = { ...options.headers };

  if (isApiMutationRequest(path, method)) {
    const token = getCookie('csrf_token');
    if (token) {
      headers['X-CSRF-Token'] = token;
    }
  }

  const res = await fetch(path, {
    credentials: 'include',
    ...options,
    headers,
  });

  let data = null;
  const contentType = res.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
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

function isApiMutationRequest(path, method) {
  if (!['POST', 'PUT', 'DELETE', 'PATCH'].includes(method)) return false;
  if (typeof path !== 'string') return false;

  // Handle both relative paths and full URLs
  let pathname = path;
  try {
    if (path.startsWith('http')) {
      pathname = new URL(path).pathname;
    }
  } catch (e) {
    /* ignore */
  }

  return pathname === '/api' || pathname.startsWith('/api/');
}

export function getCookie(name) {
  if (typeof document === 'undefined') return null;
  const value = `; ${document.cookie}`;
  const parts = value.split(`; ${name}=`);
  if (parts.length >= 2) return parts.pop().split(';').shift();
  return null;
}
