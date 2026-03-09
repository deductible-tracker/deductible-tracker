export async function apiJson(path, options = {}) {
    // CSRF Protection: Include the auth token in X-CSRF-Token header for state-changing requests
    const token = getCookie('auth_token');
    const method = (options.method || 'GET').toUpperCase();
    const headers = { ...options.headers };

    if (['POST', 'PUT', 'DELETE', 'PATCH'].includes(method)) {
        if (!token) {
            throw new Error('Missing CSRF token');
        }
        headers['X-CSRF-Token'] = token;
    }

    const res = await fetch(path, {
        credentials: 'include',
        ...options,
        headers
    });

    let data = null;
    const contentType = res.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
        try { data = await res.json(); } catch (e) { /* ignore */ }
    } else {
        try { data = await res.text(); } catch (e) { /* ignore */ }
    }

    return { res, data };
}

function getCookie(name) {
    const value = `; ${document.cookie}`;
    const parts = value.split(`; ${name}=`);
    if (parts.length === 2) return parts.pop().split(';').shift();
    return null;
}
