const CURRENT_USER_STORAGE_KEY = 'current_user';
let currentUser = null;

export function setCurrentUser(profile) {
    if (!profile || !profile.id) return;
    currentUser = profile;
    try { localStorage.setItem(CURRENT_USER_STORAGE_KEY, JSON.stringify(profile)); } catch (e) { /* ignore */ }
}

export function clearCurrentUser() {
    currentUser = null;
    try { localStorage.removeItem(CURRENT_USER_STORAGE_KEY); } catch (e) { /* ignore */ }
}

export function getCurrentUser() {
    if (currentUser) return currentUser;
    try {
        const raw = localStorage.getItem(CURRENT_USER_STORAGE_KEY);
        if (!raw) return null;
        const parsed = JSON.parse(raw);
        if (parsed && parsed.id) {
            currentUser = parsed;
            return currentUser;
        }
    } catch (e) { /* ignore */ }
    return null;
}

export function getCurrentUserId() {
    const user = getCurrentUser();
    return user ? user.id : null;
}
