export async function apiJson(path, options = {}) {
    const res = await fetch(path, {
        credentials: 'include',
        ...options
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
