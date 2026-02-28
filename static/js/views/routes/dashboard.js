export async function renderDashboardRoute(deps) {
    const {
        renderRecentList,
    } = deps;

    const root = document.getElementById('route-content') || document.getElementById('app');

    root.innerHTML = `
        <div class="mx-auto max-w-7xl">
            <div class="dt-panel overflow-hidden">
                <div class="flex items-center justify-between border-b border-slate-200 dark:border-slate-700 px-5 py-4">
                    <h2 class="text-base font-semibold text-slate-900 dark:text-slate-100">Recent activity</h2>
                </div>
                <ul class="divide-y divide-slate-100 dark:divide-slate-700" id="recent-list">
                    <li class="px-5 py-4 text-sm text-slate-500 dark:text-slate-400">Loading...</li>
                </ul>
            </div>
        </div>
    `;

    if (window.lucide) lucide.createIcons();
    renderRecentList();
}

export async function renderRecentListRoute(deps) {
    const { db, escapeHtml, getCurrentUserId, getUserCharityNameMap } = deps;
    const list = document.getElementById('recent-list');
    const userId = getCurrentUserId();
    const charityNameMap = await getUserCharityNameMap();
    const donations = (await db.donations.orderBy('date').reverse().toArray())
        .filter(d => d.user_id === userId)
        .slice(0, 5);

    if (donations.length === 0) {
        list.innerHTML = '<li class="px-5 py-4 text-sm text-slate-500 dark:text-slate-400">No donations yet.</li>';
        return;
    }

    list.innerHTML = donations.map(d => `
        <li class="px-5 py-4 transition hover:bg-slate-50 dark:bg-slate-700/50">
            <div class="flex items-center justify-between">
                <p class="truncate text-sm font-medium text-slate-900 dark:text-slate-100">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</p>
                <div class="ml-2 shrink-0 flex">
                    <p class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">
                        ${escapeHtml(d.sync_status || 'synced')}
                    </p>
                </div>
            </div>
            <div class="mt-2 text-sm text-slate-500 dark:text-slate-400">
                    <p class="flex items-center">
                        <i data-lucide="calendar" class="mr-1.5 h-4 w-4 shrink-0 text-slate-400"></i>
                        ${escapeHtml(d.date)}
                    </p>
            </div>
        </li>
    `).join('');
    lucide.createIcons();
}
