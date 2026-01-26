import db from './db.js';
import { Sync } from './sync.js';
import { seedDatabase } from './seed.js';

// Simple Router
const routes = {
    '/': renderDashboard,
    '/donations': renderDonations,
    '/reports': renderReports
};

async function navigate(path) {
    window.history.pushState({}, '', path);
    const handler = routes[path] || routes['/'];
    await handler();
    updateActiveLink(path);
}

function updateActiveLink(path) {
    document.querySelectorAll('nav a').forEach(a => {
        if (a.dataset.route === path) {
            a.classList.add('border-blue-500', 'text-gray-900');
            a.classList.remove('border-transparent', 'text-gray-500');
        } else {
            a.classList.remove('border-blue-500', 'text-gray-900');
            a.classList.add('border-transparent', 'text-gray-500');
        }
    });
}

// --- Views ---

async function renderLogin() {
    const app = document.getElementById('app');
    app.innerHTML = `
        <div class="min-h-full flex items-center justify-center py-12 px-4 sm:px-6 lg:px-8">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900">Sign in to your account</h2>
                </div>
                <form class="mt-8 space-y-6" id="login-form">
                    <input type="hidden" name="remember" value="true">
                    <div class="rounded-md shadow-sm -space-y-px">
                        <div>
                            <label for="username" class="sr-only">Username</label>
                            <input id="username" name="username" type="text" required class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-t-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 focus:z-10 sm:text-sm" placeholder="Username (e.g. user)">
                        </div>
                        <div>
                            <label for="password" class="sr-only">Password</label>
                            <input id="password" name="password" type="password" required class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-b-md focus:outline-none focus:ring-blue-500 focus:border-blue-500 focus:z-10 sm:text-sm" placeholder="Password (e.g. pass)">
                        </div>
                    </div>

                    <div>
                        <button type="submit" class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
                            Sign in
                        </button>
                    </div>
                </form>
            </div>
        </div>
    `;

    document.getElementById('login-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const username = e.target.username.value;
        const password = e.target.password.value;

        try {
            const res = await fetch('/auth/dev/login', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ username, password })
            });

            if (res.ok) {
                const { token } = await res.json();
                localStorage.setItem('jwt', token);
                document.getElementById('nav-container').classList.remove('hidden', 'sm:hidden');
                document.getElementById('auth-actions').classList.remove('hidden');
                navigate('/');
            } else {
                alert('Login failed');
            }
        } catch (err) {
            console.error(err);
            alert('Error during login');
        }
    });
}

function handleLogout() {
    localStorage.removeItem('jwt');
    document.getElementById('nav-container').classList.add('hidden');
    document.getElementById('auth-actions').classList.add('hidden');
    renderLogin();
}

async function renderDashboard() {
    const app = document.getElementById('app');
    const count = await db.donations.count();
    
    app.innerHTML = `
        <div class="max-w-7xl mx-auto">
            <h1 class="text-2xl font-bold text-gray-900">Dashboard</h1>
            <div class="mt-4 grid grid-cols-1 gap-5 sm:grid-cols-3">
                <div class="bg-white overflow-hidden shadow rounded-lg">
                    <div class="px-4 py-5 sm:p-6">
                        <dt class="text-sm font-medium text-gray-500 truncate">Total Donations (Local)</dt>
                        <dd class="mt-1 text-3xl font-semibold text-gray-900">${count}</dd>
                    </div>
                </div>
                <!-- Add more cards -->
            </div>
            
            <div class="mt-8">
                 <h2 class="text-lg font-medium text-gray-900">Recent Activity</h2>
                 <ul class="mt-4 bg-white shadow overflow-hidden rounded-md divide-y divide-gray-200" id="recent-list">
                    <li class="px-4 py-4 sm:px-6 text-gray-500 text-sm">Loading...</li>
                 </ul>
            </div>
        </div>
    `;
    
    renderRecentList();
}

async function renderRecentList() {
    const list = document.getElementById('recent-list');
    const donations = await db.donations.orderBy('date').reverse().limit(5).toArray();
    
    if (donations.length === 0) {
        list.innerHTML = '<li class="px-4 py-4 sm:px-6 text-gray-500 text-sm">No donations yet.</li>';
        return;
    }

    list.innerHTML = donations.map(d => `
        <li class="px-4 py-4 sm:px-6 hover:bg-gray-50">
            <div class="flex items-center justify-between">
                <p class="text-sm font-medium text-blue-600 truncate">${d.charity_name}</p>
                <div class="ml-2 flex-shrink-0 flex">
                    <p class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-green-100 text-green-800">
                        ${d.sync_status || 'synced'}
                    </p>
                </div>
            </div>
            <div class="mt-2 sm:flex sm:justify-between">
                <div class="sm:flex">
                    <p class="flex items-center text-sm text-gray-500">
                        <i data-lucide="calendar" class="flex-shrink-0 mr-1.5 h-4 w-4 text-gray-400"></i>
                        ${d.date}
                    </p>
                </div>
            </div>
        </li>
    `).join('');
    lucide.createIcons();
}

async function renderDonations() {
    const app = document.getElementById('app');
    const donations = await db.donations.toArray();
    
    app.innerHTML = `
        <div class="max-w-7xl mx-auto">
            <div class="flex justify-between items-center">
                <h1 class="text-2xl font-bold text-gray-900">Donations</h1>
                <button id="btn-add-fake" class="bg-blue-600 text-white px-3 py-1 rounded text-sm">Add Test Donation</button>
            </div>
            <div class="mt-4 flex flex-col">
                <div class="-my-2 overflow-x-auto sm:-mx-6 lg:-mx-8">
                    <div class="py-2 align-middle inline-block min-w-full sm:px-6 lg:px-8">
                        <div class="shadow overflow-hidden border-b border-gray-200 sm:rounded-lg">
                            <table class="min-w-full divide-y divide-gray-200">
                                <thead class="bg-gray-50">
                                    <tr>
                                        <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Date</th>
                                        <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Charity</th>
                                        <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Status</th>
                                    </tr>
                                </thead>
                                <tbody class="bg-white divide-y divide-gray-200">
                                    ${donations.map(d => `
                                        <tr>
                                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">${d.date}</td>
                                            <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">${d.charity_name}</td>
                                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">${d.sync_status || 'synced'}</td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    `;
    
    document.getElementById('btn-add-fake').addEventListener('click', async () => {
        const id = crypto.randomUUID();
        const d = {
            id,
            year: 2026,
            date: new Date().toISOString().split('T')[0],
            charity_name: 'Goodwill Industries',
            charity_id: '12-3456789',
            notes: 'Test donation',
            sync_status: 'created'
        };
        await Sync.queueAction('donations', d, 'create');
        renderDonations(); // Refresh
    });
}

function renderReports() {
    document.getElementById('app').innerHTML = '<div class="max-w-7xl mx-auto"><h1 class="text-2xl font-bold">Reports (Coming Soon)</h1></div>';
}

// --- Init ---
async function init() {
    console.log('App initializing...');
    
    // 1. Network Status & Initial UI State
    const updateStatus = () => {
        const isOnline = navigator.onLine;
        const statusEl = document.getElementById('sync-status');
        if (!statusEl) return;
        
        if (isOnline) {
            statusEl.innerHTML = '<i data-lucide="cloud" class="h-4 w-4 mr-1 text-green-500"></i> Online';
            Sync.pushChanges().catch(err => console.error('Initial sync failed:', err));
        } else {
            statusEl.innerHTML = '<i data-lucide="cloud-off" class="h-4 w-4 mr-1 text-red-500"></i> Offline';
        }
        if (window.lucide) lucide.createIcons();
    };

    window.addEventListener('online', updateStatus);
    window.addEventListener('offline', updateStatus);
    updateStatus();

    // 2. Global Event Listeners (Attach immediately)
    document.querySelectorAll('nav a').forEach(a => {
        a.addEventListener('click', (e) => {
            e.preventDefault();
            const link = e.target.closest('a');
            const route = link ? link.dataset.route : null;
            if (route) navigate(route);
        });
    });

    const btnAddDonation = document.getElementById('btn-add-donation');
    if (btnAddDonation) {
        btnAddDonation.addEventListener('click', () => navigate('/donations'));
    }

    const btnLogout = document.getElementById('btn-logout');
    if (btnLogout) {
        btnLogout.addEventListener('click', handleLogout);
    }

    try {
        // 3. Database Seeding
        console.log('Seeding database...');
        await seedDatabase();
        
        // 4. Auth Check
        const token = localStorage.getItem('jwt');
        if (!token) {
            console.log('No token found, rendering login');
            document.getElementById('nav-container').classList.add('hidden');
            document.getElementById('auth-actions').classList.add('hidden');
            renderLogin();
        } else {
            console.log('Token found, navigating to route');
            document.getElementById('nav-container').classList.remove('hidden', 'sm:hidden');
            document.getElementById('auth-actions').classList.remove('hidden');
            // Initial Route
            const initialRoute = location.pathname === '/index.html' || location.pathname === '/' 
                ? '/' 
                : location.pathname;
            await navigate(initialRoute);
        }
    } catch (err) {
        console.error('Initialization failed:', err);
        // Fallback: show something so the user isn't stuck at "Loading..."
        document.getElementById('app').innerHTML = `
            <div class="p-8 text-center">
                <h1 class="text-red-600 font-bold">Initialization Error</h1>
                <p class="text-gray-600">${err.message}</p>
                <button onclick="location.reload()" class="mt-4 bg-blue-600 text-white px-4 py-2 rounded">Retry</button>
            </div>
        `;
    }
}

// Run init when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
