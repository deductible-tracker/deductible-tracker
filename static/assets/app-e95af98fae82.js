import db from'./db-c260a5e79074.js';import{Sync}from'./sync-59e295466f4b.js';import{seedDatabase}from'./seed-8d5ea2f0491f.js';const routes={'/':renderDashboard,'/donations':renderDonations,'/charities':renderCharities,'/reports':renderReports,'/personal':renderPersonalInfo};let AUTHENTICATED=false;let RETURN_TO=null;const CURRENT_USER_STORAGE_KEY='current_user';const CHARITY_CACHE_TTL_MS=1000*60*60*24*30;let currentUser=null;let lastAuthCheckAt=0;let lastAuthResult=false;let authCheckInFlight=null;let lastUserFetchAt=0;let pendingDonationCategory=null;let pendingCharityEditId=null;function setCurrentUser(profile){if(!profile||!profile.id)return;currentUser=profile;try{localStorage.setItem(CURRENT_USER_STORAGE_KEY,JSON.stringify(profile));}catch(e){}}function clearCurrentUser(){currentUser=null;try{localStorage.removeItem(CURRENT_USER_STORAGE_KEY);}catch(e){}}function getCurrentUser(){if(currentUser)return currentUser;try{const raw=localStorage.getItem(CURRENT_USER_STORAGE_KEY);if(!raw)return null;const parsed=JSON.parse(raw);if(parsed&&parsed.id){currentUser=parsed;return currentUser;}}catch(e){}return null;}function getCurrentUserId(){const user=getCurrentUser();return user?user.id:null;}function getProfileStorageKey(){const userId=getCurrentUserId();return userId?`profile:${userId}`:'profile:anonymous';}async function clearUserCaches(){try{await db.donations.clear();}catch(e){}try{await db.receipts.clear();}catch(e){}try{await db.charities.clear();}catch(e){}}async function checkAuthCached(){const now=Date.now();const ttlMs=4000;if(authCheckInFlight)return authCheckInFlight;if(now-lastAuthCheckAt<ttlMs)return lastAuthResult;authCheckInFlight=(async()=>{try{const res=await fetch('/api/me',{credentials:'include'});if(res.status===429)return lastAuthResult;lastAuthResult=res.ok;lastAuthCheckAt=Date.now();if(res.ok){const shouldFetchUser=now-lastUserFetchAt>ttlMs||!getCurrentUserId();if(shouldFetchUser){try{const profile=await res.json();setCurrentUser(profile);lastUserFetchAt=Date.now();}catch(e){}}}else{clearCurrentUser();}return lastAuthResult;}catch(e){return lastAuthResult;}finally{authCheckInFlight=null;}})();return authCheckInFlight;}const escapeHtml=(value)=>{if(value===null||value===undefined)return'';return String(value).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#039;');};async function apiJson(path,options={}){const res=await fetch(path,{credentials:'include',...options});let data=null;const contentType=res.headers.get('content-type')||'';if(contentType.includes('application/json')){try{data=await res.json();}catch(e){}}else{try{data=await res.text();}catch(e){}}return{res,data};}async function createOrGetCharityOnServer(nameOrPayload,ein){const payload=(typeof nameOrPayload==='object'&&nameOrPayload!==null)?nameOrPayload:{name:nameOrPayload,ein};const{res,data}=await apiJson('/api/charities',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)});if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to create charity');}return data;}async function lookupCharityByEinOnServer(ein){const normalizedEin=(ein||'').replace(/\D/g,'');if(!normalizedEin)return null;const{res,data}=await apiJson(`/api/charities/lookup/${encodeURIComponent(normalizedEin)}`);if(!res.ok)return null;return data&&data.charity?data.charity:null;}async function updateCharityOnServer(charityId,payload){const{res,data}=await apiJson(`/api/charities/${encodeURIComponent(charityId)}`,{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)});if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to update charity');}return data;}async function fetchCharitiesFromServer(){const{res,data}=await apiJson('/api/charities');if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to fetch charities');}return(data&&data.charities)?data.charities:[];}async function deleteCharityOnServer(charityId){const{res,data}=await apiJson(`/api/charities/${encodeURIComponent(charityId)}`,{method:'DELETE'});if(res.status===409){throw new Error('Charity has donations and cannot be deleted');}if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to delete charity');}}async function createDonationOnServer(payload){const{res,data}=await apiJson('/api/donations',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)});if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to create donation');}return data;}async function deleteDonationOnServer(donationId){const{res,data}=await apiJson(`/api/donations/${encodeURIComponent(donationId)}`,{method:'DELETE'});if(!res.ok){throw new Error(typeof data==='string'?data:'Failed to delete donation');}}async function refreshDonationsFromServer(){const userId=getCurrentUserId();if(!userId)return;const{res,data}=await apiJson('/api/donations');if(!res.ok||!data||!data.donations)return;const donations=data.donations.map(d=>({id:d.id,user_id:userId,year:d.year,date:d.date,category:d.category||'money',amount:d.amount??0,charity_id:d.charity_id,notes:d.notes||null,sync_status:'synced',updated_at:d.updated_at||null,created_at:d.created_at||null}));try{await db.donations.where('user_id').equals(userId).delete();await db.donations.bulkPut(donations);}catch(e){}}async function refreshReceiptsFromServer(){const userId=getCurrentUserId();if(!userId)return;try{const{res,data}=await apiJson('/api/receipts');if(!res.ok||!data||!data.receipts)return;const receipts=data.receipts.map(r=>({id:r.id,key:r.key,file_name:r.file_name||null,content_type:r.content_type||null,size:r.size||null,donation_id:r.donation_id,uploaded_at:r.created_at||new Date().toISOString()}));try{await db.receipts.clear();await db.receipts.bulkPut(receipts);}catch(e){}}catch(e){console.error('Failed to refresh receipts',e);}}async function refreshCharitiesCache(){const userId=getCurrentUserId();if(!userId)return[];const list=await fetchCharitiesFromServer();const now=Date.now();const cached=list.map(c=>({id:c.id,user_id:userId,name:c.name,ein:c.ein||'',category:c.category||null,status:c.status||null,classification:c.classification||null,nonprofit_type:c.nonprofit_type||null,deductibility:c.deductibility||null,street:c.street||null,city:c.city||null,state:c.state||null,zip:c.zip||null,cached_at:now}));try{await db.charities.where('user_id').equals(userId).delete();await db.charities.bulkPut(cached);}catch(e){}return cached;}function normalizeRoute(path){const raw=(path||'/').toString();const pathname=raw.split('?')[0]||'/';if(pathname==='/index.html')return'/';return routes[pathname]?pathname:'/';}function updateHomeSummaryVisibility(path){const summary=document.getElementById('home-summary');if(!summary)return;const routeContent=document.getElementById('route-content');const isHome=normalizeRoute(path)==='/';summary.classList.toggle('hidden',!isHome);if(routeContent){routeContent.classList.toggle('mt-8',isHome);routeContent.classList.toggle('mt-0',!isHome);}}async function navigate(path,options={}){const{pushState=true}=options;const target=normalizeRoute(path);if(!AUTHENTICATED){RETURN_TO=target;renderLogin();return;}if(pushState){window.history.pushState({},'',target);}updateHomeSummaryVisibility(target);const handler=routes[target]||routes['/'];await handler();updateActiveLink(target);}function updateActiveLink(path){document.querySelectorAll('nav a').forEach(a=>{if(a.dataset.route===path){a.classList.add('bg-slate-900','text-white');a.classList.remove('text-slate-700','hover:bg-slate-100');}else{a.classList.remove('bg-slate-900','text-white');a.classList.add('text-slate-700','hover:bg-slate-100');}});}async function renderLogin(){const app=document.getElementById('app');app.innerHTML=`
        <div class="mx-auto grid min-h-full max-w-6xl items-center gap-8 py-8 sm:py-12 lg:grid-cols-2">
            <div class="rounded-2xl border border-slate-200 bg-linear-to-br from-indigo-50 to-white p-6 sm:p-10">
                <p class="text-xs font-semibold uppercase tracking-[0.14em] text-indigo-600">Deductible Tracker</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-slate-900 sm:text-4xl">A calmer way to track charitable giving</h1>
                <p class="mt-4 max-w-xl text-sm text-slate-600 sm:text-base">Capture donations, keep receipts attached, and export polished yearly reports from one clean workspace.</p>
                <div class="mt-6 grid gap-3 text-sm text-slate-600 sm:grid-cols-2">
                    <div class="rounded-xl border border-slate-200 bg-white px-4 py-3">Fast donation entry</div>
                    <div class="rounded-xl border border-slate-200 bg-white px-4 py-3">Receipt management</div>
                    <div class="rounded-xl border border-slate-200 bg-white px-4 py-3">Offline-first sync</div>
                    <div class="rounded-xl border border-slate-200 bg-white px-4 py-3">CSV exports</div>
                </div>
            </div>
            <div class="dt-panel p-6 sm:p-8">
                <h2 class="text-2xl font-semibold text-slate-900">Sign in</h2>
                <p class="mt-1 text-sm text-slate-600">Use your account credentials to continue.</p>
                <form class="mt-6 space-y-4" id="login-form">
                    <input type="hidden" name="remember" value="true" />
                    <div>
                        <label for="username" class="dt-label">Username</label>
                        <input id="username" name="username" type="text" required class="dt-input" placeholder="Username" />
                    </div>
                    <div>
                        <label for="password" class="dt-label">Password</label>
                        <input id="password" name="password" type="password" required class="dt-input" placeholder="Password" />
                    </div>
                    <div class="pt-2">
                        <button type="submit" class="dt-btn-primary w-full">
                            Sign in
                        </button>
                    </div>
                </form>
            </div>
        </div>
    `;document.getElementById('login-form').addEventListener('submit',async(e)=>{e.preventDefault();const username=e.target.username.value;const password=e.target.password.value;try{const res=await fetch('/auth/dev/login',{method:'POST',headers:{'Content-Type':'application/json'},credentials:'include',body:JSON.stringify({username,password})});if(res.ok){let profile=null;try{const body=await res.json();profile=body&&body.user?body.user:body;}catch(e){}const previousUserId=getCurrentUserId();if(profile)setCurrentUser(profile);const nextUserId=profile&&profile.id?profile.id:null;if(!previousUserId||(nextUserId&&nextUserId!==previousUserId)){await clearUserCaches();}AUTHENTICATED=true;document.getElementById('nav-container').classList.remove('hidden','sm:hidden');document.getElementById('auth-actions').classList.remove('hidden');try{await Sync.pushChanges();}catch(e){console.warn('Initial push changes failed',e);}try{await refreshCharitiesCache();}catch(e){console.warn('Failed to refresh charities on login',e);}try{await refreshDonationsFromServer();}catch(e){console.warn('Failed to refresh donations on login',e);}try{await refreshReceiptsFromServer();}catch(e){console.warn('Failed to refresh receipts on login',e);}await updateTotals();const goto=RETURN_TO||'/';RETURN_TO=null;await navigate(goto);}else{alert('Login failed');}}catch(err){console.error(err);alert('Error during login');}});}async function getUserDonations(){const userId=getCurrentUserId();if(!userId)return[];return db.donations.where('user_id').equals(userId).toArray();}async function getUserCharityNameMap(){const userId=getCurrentUserId();const map=new Map();if(!userId)return map;const charities=await db.charities.where('user_id').equals(userId).toArray();for(const charity of charities){if(charity&&charity.id){map.set(charity.id,charity.name||'Unknown charity');}}return map;}function isCharityCacheFresh(entry){if(!entry||!entry.cached_at)return false;return Date.now()-entry.cached_at<=CHARITY_CACHE_TTL_MS;}async function handleLogout(){try{await fetch('/auth/logout',{method:'POST',credentials:'include'});}catch(e){console.warn('Logout request failed',e);}const nav=document.getElementById('nav-container');if(nav)nav.classList.add('hidden');const authActions=document.getElementById('auth-actions');if(authActions)authActions.classList.add('hidden');AUTHENTICATED=false;const profileKey=getProfileStorageKey();await clearUserCaches();try{localStorage.removeItem(profileKey);}catch(e){}clearCurrentUser();lastAuthResult=false;lastAuthCheckAt=0;lastUserFetchAt=0;renderLogin();try{window.location.replace('/');}catch(e){}}async function renderDashboard(){const root=document.getElementById('route-content')||document.getElementById('app');const donations=await getUserDonations();const figures=calculateDonationFigures(donations);root.innerHTML=`
        <div class="mx-auto max-w-7xl space-y-6">
            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <div class="dt-panel p-5">
                    <div class="text-xs font-medium uppercase tracking-wide text-slate-500">Donations</div>
                    <div class="mt-3 text-3xl font-semibold text-slate-900">${escapeHtml(formatFigureText(figures.total))}</div>
                </div>
                <div class="dt-panel p-5">
                    <div class="text-xs font-medium uppercase tracking-wide text-slate-500">Items</div>
                    <div class="mt-3 text-3xl font-semibold text-slate-900">${escapeHtml(formatFigureText(figures.items))}</div>
                </div>
                <div class="dt-panel p-5">
                    <div class="text-xs font-medium uppercase tracking-wide text-slate-500">Money</div>
                    <div class="mt-3 text-3xl font-semibold text-slate-900">${escapeHtml(formatFigureText(figures.money))}</div>
                </div>
                <div class="dt-panel p-5">
                    <div class="text-xs font-medium uppercase tracking-wide text-slate-500">Mileage</div>
                    <div class="mt-3 text-3xl font-semibold text-slate-900">${escapeHtml(formatFigureText(figures.mileage))}</div>
                </div>
            </div>
            <div class="dt-panel overflow-hidden">
                 <div class="flex items-center justify-between border-b border-slate-200 px-5 py-4">
                    <h2 class="text-base font-semibold text-slate-900">Recent activity</h2>
                    <button class="dt-btn-secondary" id="btn-add-donation-inline-home">Add donation</button>
                 </div>
                 <ul class="divide-y divide-slate-100" id="recent-list">
                    <li class="px-5 py-4 text-sm text-slate-500">Loading...</li>
                 </ul>
            </div>
        </div>
    `;const btnAddInlineHome=document.getElementById('btn-add-donation-inline-home');if(btnAddInlineHome)btnAddInlineHome.addEventListener('click',()=>openAddDonationModal());renderRecentList();await updateTotals();}async function renderRecentList(){const list=document.getElementById('recent-list');const userId=getCurrentUserId();const charityNameMap=await getUserCharityNameMap();const donations=(await db.donations.orderBy('date').reverse().toArray()).filter(d=>d.user_id===userId).slice(0,5);if(donations.length===0){list.innerHTML='<li class="px-5 py-4 text-sm text-slate-500">No donations yet.</li>';return;}list.innerHTML=donations.map(d=>`
        <li class="px-5 py-4 transition hover:bg-slate-50">
            <div class="flex items-center justify-between">
                <p class="truncate text-sm font-medium text-slate-900">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</p>
                <div class="ml-2 shrink-0 flex">
                    <p class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700">
                        ${escapeHtml(d.sync_status || 'synced')}
                    </p>
                </div>
            </div>
            <div class="mt-2 text-sm text-slate-500">
                    <p class="flex items-center">
                        <i data-lucide="calendar" class="mr-1.5 h-4 w-4 shrink-0 text-slate-400"></i>
                        ${escapeHtml(d.date)}
                    </p>
            </div>
        </li>
    `).join('');lucide.createIcons();}async function renderDonations(){const root=document.getElementById('route-content')||document.getElementById('app');const userId=getCurrentUserId();const charities=userId?await db.charities.where('user_id').equals(userId).toArray():[];const charityNameMap=new Map(charities.map(c=>[c.id,c.name||'Unknown charity']));const donations=userId?(await db.donations.where('user_id').equals(userId).toArray()).sort((a,b)=>String(b.date||'').localeCompare(String(a.date||''))):[];const receipts=await db.receipts.toArray();const taxEstimates=await calculateTaxEstimates(donations,charities,receipts);const categoryPrefill=pendingDonationCategory&&['items','money','mileage'].includes(pendingDonationCategory)?pendingDonationCategory:'money';pendingDonationCategory=null;root.innerHTML=`
        <div class="mx-auto max-w-7xl space-y-5">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900">Donations</h1>
                    <p class="mt-1 text-sm text-slate-600">Add donations, attach receipts immediately, and keep records audit-ready.</p>
                </div>
            </div>
            <div class="dt-panel p-5 sm:p-6">
                <h2 class="text-base font-semibold text-slate-900">Add Donation</h2>
                <form id="donation-page-form" class="mt-4 space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Donation Date</label>
                            <input id="donation-date" type="date" required class="dt-input" />
                        </div>
                        <div>
                            <label class="dt-label">Category</label>
                            <select id="donation-category" class="dt-input">
                                <option value="items" ${categoryPrefill === 'items' ? 'selected' : ''}>Items</option>
                                <option value="money" ${categoryPrefill === 'money' ? 'selected' : ''}>Money</option>
                                <option value="mileage" ${categoryPrefill === 'mileage' ? 'selected' : ''}>Mileage</option>
                            </select>
                        </div>
                    </div>
                    <div>
                        <label class="dt-label">Charity</label>
                        <input id="donation-charity-input" type="text" required placeholder="Search or type to add" class="dt-input" autocomplete="off" />
                        <input id="donation-charity-id" type="hidden" />
                        <input id="donation-charity-ein" type="hidden" />
                        <div id="charity-suggestions" class="mt-1 hidden max-h-48 overflow-auto rounded-xl border border-slate-200 bg-white"></div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Amount</label>
                            <input id="donation-amount" type="number" step="0.01" class="dt-input" />
                        </div>
                        <div>
                            <label class="dt-label">Receipts</label>
                            <input id="donation-receipts" type="file" multiple accept="image/*,application/pdf" class="dt-input" />
                        </div>
                    </div>
                    <div>
                        <label class="dt-label">Notes</label>
                        <textarea id="donation-notes" rows="3" class="dt-input"></textarea>
                    </div>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">Save Donation</button>
                    </div>
                </form>
            </div>
            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200">
                                <thead class="bg-slate-50">
                                    <tr>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Date</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Charity</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Status</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Category</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Amount</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Estimated Tax Savings</th>
                                        <th scope="col" class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Actions</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-slate-100 bg-white">
                                    ${donations.length === 0 ? `
                                        <tr>
                                            <td colspan="7" class="px-5 py-8 text-sm text-slate-500">No donations yet.</td>
                                        </tr>
                                    ` : donations.map(d => `
                                        <tr class="hover:bg-slate-50/70">
                                                <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600">${escapeHtml(d.date)}</td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-slate-900">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600">
                                                    <span class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700">${escapeHtml(d.sync_status || 'synced')}</span>
                                                </td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600">${escapeHtml(d.category || '')}</td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-slate-900">${d.amount ? `$${parseFloat(d.amount).toFixed(2)}` : ''}</td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm font-medium text-emerald-700">${formatCurrency(taxEstimates.perDonation.get(d.id) || 0)}</td>
                                                <td class="whitespace-nowrap px-5 py-4 text-sm text-slate-600">
                                                    <button class="manage-receipts-btn dt-btn-secondary px-3 py-1.5" data-id="${d.id}">Receipts</button>
                                                    <button class="delete-donation-btn dt-btn-danger ml-2 px-3 py-1.5" data-id="${d.id}">Delete</button>
                                                </td>
                                        </tr>
                                    `).join('')}
                                </tbody>
                            </table>
                </div>
                <div class="space-y-3 p-4 md:hidden">
                    ${donations.length === 0 ? '<div class="rounded-xl border border-slate-200 bg-white p-4 text-sm text-slate-500">No donations yet.</div>' : donations.map(d => `
                        <article class="rounded-xl border border-slate-200 bg-white p-4">
                            <div class="flex items-start justify-between gap-3">
                                <div>
                                    <p class="text-sm font-semibold text-slate-900">${escapeHtml(charityNameMap.get(d.charity_id) || 'Unknown charity')}</p>
                                    <p class="mt-1 text-xs text-slate-500">${escapeHtml(d.date || '')} • ${escapeHtml(d.category || '')}</p>
                                </div>
                                <span class="inline-flex rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-700">${escapeHtml(d.sync_status || 'synced')}</span>
                            </div>
                            <div class="mt-3 grid grid-cols-2 gap-2 text-sm">
                                <div class="rounded-lg bg-slate-50 px-3 py-2">
                                    <p class="text-xs text-slate-500">Amount</p>
                                    <p class="font-semibold text-slate-900">${d.amount ? `$${parseFloat(d.amount).toFixed(2)}` : '$0.00'}</p>
                                </div>
                                <div class="rounded-lg bg-slate-50 px-3 py-2">
                                    <p class="text-xs text-slate-500">Est. savings</p>
                                    <p class="font-semibold text-emerald-700">${formatCurrency(taxEstimates.perDonation.get(d.id) || 0)}</p>
                                </div>
                            </div>
                            <div class="mt-3 flex gap-2">
                                <button class="manage-receipts-btn dt-btn-secondary flex-1 px-3 py-1.5" data-id="${d.id}">Receipts</button>
                                <button class="delete-donation-btn dt-btn-danger flex-1 px-3 py-1.5" data-id="${d.id}">Delete</button>
                            </div>
                        </article>
                    `).join('')}
                </div>
            </div>
        </div>
    `;const dateInput=document.getElementById('donation-date');if(dateInput&&!dateInput.value){dateInput.value=new Date().toISOString().split('T')[0];}const form=document.getElementById('donation-page-form');const charityInput=document.getElementById('donation-charity-input');const charityIdInput=document.getElementById('donation-charity-id');const charityEinInput=document.getElementById('donation-charity-ein');const suggestionsBox=document.getElementById('charity-suggestions');let suggestionTimer=null;charityInput?.addEventListener('input',(e)=>{charityIdInput.value='';charityEinInput.value='';const q=e.target.value.trim();if(suggestionTimer)clearTimeout(suggestionTimer);if(!q){suggestionsBox.innerHTML='';suggestionsBox.classList.add('hidden');return;}suggestionTimer=setTimeout(async()=>{try{const qLower=q.toLowerCase();let localMatches=[];try{localMatches=await db.charities.where('user_id').equals(userId).filter(c=>isCharityCacheFresh(c)&&(c.name||'').toLowerCase().includes(qLower)).toArray();}catch(le){console.warn('Local charity lookup failed',le);}let remote=[];try{const res=await fetch(`/api/charities/search?q=${encodeURIComponent(q)}`,{credentials:'include'});if(res.ok){const data=await res.json();remote=data.results||[];}}catch(re){console.warn('Remote charity search failed',re);}const seen=new Set();const merged=[];for(const c of localMatches){const key=((c.ein||'').trim()||(c.name||'').trim()).toLowerCase();if(!seen.has(key)){seen.add(key);merged.push({id:c.id,ein:c.ein||'',name:c.name,location:'',source:'local'});}}for(const r of remote){const key=((r.ein||'').trim()||(r.name||'').trim()).toLowerCase();if(!seen.has(key)){seen.add(key);merged.push({id:'',ein:r.ein||'',name:r.name,location:r.location||'',source:'remote'});}}if(merged.length===0){suggestionsBox.innerHTML='<div class="p-2 text-sm text-slate-500">No matches</div>';suggestionsBox.classList.remove('hidden');return;}suggestionsBox.innerHTML=merged.map(r=>`
                    <div class="flex cursor-pointer items-center justify-between p-2 hover:bg-slate-50" data-id="${escapeHtml(r.id || '')}" data-ein="${escapeHtml(r.ein)}" data-name="${escapeHtml(r.name)}">
                        <div>
                            <div class="font-medium text-slate-900">${escapeHtml(r.name)}</div>
                            <div class="text-xs text-slate-400">${escapeHtml(r.location || (r.source === 'local' ? 'Cached' : ''))}</div>
                        </div>
                        <div class="ml-4 text-xs text-slate-500">${r.source === 'local' ? 'Saved' : ''}</div>
                    </div>
                `).join('');suggestionsBox.classList.remove('hidden');suggestionsBox.querySelectorAll('div[data-id][data-ein][data-name]').forEach(el=>{el.addEventListener('click',()=>{const ein=el.dataset.ein||'';const name=el.dataset.name||'';const id=el.dataset.id||'';charityInput.value=name;charityIdInput.value=id;charityEinInput.value=ein;suggestionsBox.classList.add('hidden');});});}catch(err){console.error('Charity search failed',err);suggestionsBox.classList.add('hidden');}},300);});form?.addEventListener('submit',async(e)=>{e.preventDefault();if(!userId){alert('Please sign in again');return;}const date=document.getElementById('donation-date').value;const charity_name=charityInput.value.trim();const charity_id=charityIdInput.value.trim();const charity_ein=charityEinInput.value.trim();const notes=document.getElementById('donation-notes').value.trim();const category=document.getElementById('donation-category').value;const amount=parseFloat(document.getElementById('donation-amount').value)||0;const receiptFiles=Array.from((document.getElementById('donation-receipts').files||[]));if(!date||!charity_name){alert('Please provide date and charity name');return;}const fallbackId=crypto.randomUUID();const year=new Date(date).getFullYear();try{let charityId=charity_id||'';if(!charityId){const resp=await createOrGetCharityOnServer(charity_name,charity_ein||null);const charity=resp&&resp.charity?resp.charity:null;if(charity&&charity.id){charityId=charity.id;await db.charities.put({id:charity.id,user_id:userId,name:charity.name,ein:charity.ein||'',category:charity.category||null,status:charity.status||null,classification:charity.classification||null,nonprofit_type:charity.nonprofit_type||null,deductibility:charity.deductibility||null,street:charity.street||null,city:charity.city||null,state:charity.state||null,zip:charity.zip||null,cached_at:Date.now()});}}const payload={date,charity_name,charity_id:charityId||null,charity_ein:charity_ein||null,category,amount,notes:notes||null};let donation;try{const res=await createDonationOnServer(payload);const serverId=res&&res.id?res.id:fallbackId;donation={id:serverId,user_id:userId,year,date,charity_id:charityId||null,notes:notes||null,category,amount,sync_status:'synced'};await db.donations.put(donation);}catch(err){donation={id:fallbackId,user_id:userId,year,date,charity_id:charityId||null,notes:notes||null,category,amount,sync_status:'pending'};await Sync.queueAction('donations',donation,'create');}if(receiptFiles.length>0&&donation.sync_status==='synced'){for(const file of receiptFiles){await uploadReceiptForDonation(file,donation.id);}}else if(receiptFiles.length>0&&donation.sync_status!=='synced'){alert('Donation saved offline. Upload receipts after sync completes.');}await renderDonations();await updateTotals();}catch(err){console.error('Failed to create donation',err);alert('Failed to save donation');}});document.querySelectorAll('.manage-receipts-btn').forEach(btn=>{btn.addEventListener('click',(e)=>{const id=e.currentTarget.dataset.id;openReceiptManager(id);});});document.querySelectorAll('.delete-donation-btn').forEach(btn=>{btn.addEventListener('click',async(e)=>{const id=e.currentTarget.dataset.id;if(!confirm('Delete this donation?'))return;try{await deleteDonationOnServer(id);await db.donations.delete(id);await renderDonations();await updateTotals();}catch(err){console.error('Delete failed',err);alert('Failed to delete donation');}});});}async function openReceiptManager(donationId){const modal=document.createElement('div');modal.className='dt-modal-wrap';modal.innerHTML=`
        <div class="dt-modal max-w-3xl max-h-[88vh] overflow-y-auto">
            <div class="flex justify-between items-center mb-4">
                <h3 class="text-lg font-semibold text-slate-900">Manage Receipts</h3>
                <button id="modal-close" class="dt-btn-secondary px-3 py-1.5">Close</button>
            </div>
            <div id="attached-section" class="mb-6">
                <h4 class="text-sm font-semibold uppercase tracking-wide text-slate-500">Attached Receipts</h4>
                <div id="attached-list" class="mt-2"></div>
            </div>
        </div>
    `;document.body.appendChild(modal);document.getElementById('modal-close').addEventListener('click',()=>{document.body.removeChild(modal);});async function refreshLists(){const normalizedDonationId=(donationId&&String(donationId).trim())?String(donationId):null;if(!normalizedDonationId){return;}const attached=await db.receipts.where('donation_id').equals(normalizedDonationId).toArray();const attachedList=modal.querySelector('#attached-list');attachedList.innerHTML=attached.length===0?'<div class="dt-panel-muted px-3 py-2 text-sm text-slate-500">No attached receipts.</div>':attached.map(r=>`
            <div class="mb-2 flex items-center justify-between rounded-xl border border-slate-200 p-3">
                <div>
                    <div class="text-sm font-medium text-slate-900">${escapeHtml(r.file_name || r.key)}</div>
                    <div class="text-xs text-slate-500">${new Date(r.uploaded_at).toLocaleString()}</div>
                </div>
                <div class="flex items-center space-x-2">
                    <button class="preview-receipt-btn dt-btn-secondary px-3 py-1.5" data-key="${r.key}">Preview</button>
                </div>
            </div>
        `).join('');modal.querySelectorAll('.preview-receipt-btn').forEach(b=>{b.addEventListener('click',async(e)=>{const key=e.currentTarget.dataset.key;try{const downloadUrl=await getReceiptDownloadUrl(key);window.open(downloadUrl,'_blank');}catch(err){alert('Preview failed');}});});}try{await refreshLists();}catch(err){console.error('Failed to load receipts',err);alert('Unable to load receipts right now. Please try again.');}}async function getReceiptDownloadUrl(key){const res=await fetch('/api/receipts/presign',{method:'POST',headers:{'Content-Type':'application/json'},credentials:'include',body:JSON.stringify({key})});if(!res.ok)throw new Error('Presign failed');const data=await res.json();return data.download_url;}async function openAddDonationModal(initialCategory=null){if(initialCategory&&['items','money','mileage'].includes(initialCategory)){pendingDonationCategory=initialCategory;}await navigate('/donations');const form=document.getElementById('donation-page-form');if(form){form.scrollIntoView({behavior:'smooth',block:'start'});}}async function openAddCharityModal(existingCharity=null){pendingCharityEditId=existingCharity&&existingCharity.id?existingCharity.id:null;await navigate('/charities');const form=document.getElementById('charity-page-form');if(form){form.scrollIntoView({behavior:'smooth',block:'start'});}}async function renderReports(){const root=document.getElementById('route-content')||document.getElementById('app');let years=[];try{const{res,data}=await apiJson('/api/reports/years');if(res.ok&&data&&Array.isArray(data.years)){years=data.years.filter(y=>Number.isInteger(y)).sort((a,b)=>b-a);}}catch(e){console.warn('Failed to load report years',e);}const hasDonationYears=years.length>0;const currentYear=new Date().getFullYear();if(years.length===0)years=[currentYear];const optionsHtml=years.map(y=>`<option value="${y}">${y}</option>`).join('');root.innerHTML=`
        <div class="mx-auto max-w-7xl space-y-4">
            <div class="flex items-end justify-between gap-4">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900">Reports</h1>
                    <p class="mt-1 text-sm text-slate-600">Generate donation exports for your tax records.</p>
                </div>
                <div class="flex items-end gap-2">
                    <div>
                        <label for="export-year" class="dt-label">Year</label>
                        <select id="export-year" class="dt-input mt-1 w-32">${optionsHtml}</select>
                    </div>
                    <button id="btn-export-csv" class="dt-btn-primary">Export CSV</button>
                    <button id="btn-export-tax-txf" class="dt-btn-secondary">Export TXF</button>
                </div>
            </div>
            <div class="dt-panel p-6">
                <p class="text-sm text-slate-600">Select a donation year and export donations as CSV or TXF.</p>
            </div>
        </div>
    `;const yearEl=document.getElementById('export-year');const csvBtn=document.getElementById('btn-export-csv');const taxTxfBtn=document.getElementById('btn-export-tax-txf');if(!hasDonationYears){csvBtn.disabled=true;taxTxfBtn.disabled=true;}const downloadReport=async(endpoint,extension)=>{const year=yearEl.value;try{const res=await fetch(`${endpoint}?year=${encodeURIComponent(year)}`,{credentials:'include'});if(!res.ok)throw new Error('Export failed');const blob=await res.blob();const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=`donations-${year}.${extension}`;document.body.appendChild(a);a.click();a.remove();URL.revokeObjectURL(url);}catch(e){console.error(e);alert('Export failed');}};csvBtn.addEventListener('click',async()=>{await downloadReport('/api/reports/export','csv');});taxTxfBtn.addEventListener('click',async()=>{await downloadReport('/api/reports/export/txf','txf');});}async function renderCharities(){const root=document.getElementById('route-content')||document.getElementById('app');const userId=getCurrentUserId();let charities=[];try{if(userId){charities=await refreshCharitiesCache();}}catch(e){charities=userId?await db.charities.where('user_id').equals(userId).toArray():[];}const formatAddress=(charity)=>{const parts=[charity.street,charity.city,charity.state,charity.zip].map(v=>(v||'').trim()).filter(Boolean);return parts.length?parts.join(', '):'—';};const existing=pendingCharityEditId?charities.find(c=>c.id===pendingCharityEditId):null;const isEditMode=!!existing;root.innerHTML=`
        <div class="mx-auto max-w-7xl space-y-5">
            <div>
                <h1 class="text-2xl font-semibold text-slate-900">Charities</h1>
                <p class="mt-1 text-sm text-slate-600">Manage your nonprofit directory and keep compliance details handy.</p>
            </div>
            <div class="dt-panel p-5 sm:p-6">
                <div class="flex items-center justify-between">
                    <h2 class="text-base font-semibold text-slate-900">${isEditMode ? 'Edit Charity' : 'Add Charity'}</h2>
                    ${isEditMode ? '<button id="btn-cancel-charity-edit" class="dt-btn-secondary">Cancel</button>' : ''}
                </div>
                <form id="charity-page-form" class="mt-4 space-y-4">
                    <div>
                        <label class="dt-label">Name</label>
                        <input id="charity-name" required class="dt-input" value="${escapeHtml(existing?.name || '')}" autocomplete="off" />
                        <div id="charity-name-suggestions" class="mt-1 hidden max-h-48 overflow-auto rounded-xl border border-slate-200 bg-white"></div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">EIN</label>
                            <input id="charity-ein" class="dt-input" value="${escapeHtml(existing?.ein || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Type of Nonprofit</label>
                            <input id="charity-nonprofit-type" class="dt-input" value="${escapeHtml(existing?.nonprofit_type || '')}" placeholder="e.g. 501(c)(3)" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Category</label>
                            <input id="charity-category" class="dt-input" value="${escapeHtml(existing?.category || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Status</label>
                            <input id="charity-status" class="dt-input" value="${escapeHtml(existing?.status || '')}" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Classification</label>
                            <input id="charity-classification" class="dt-input" value="${escapeHtml(existing?.classification || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Deductibility</label>
                            <input id="charity-deductibility" class="dt-input" value="${escapeHtml(existing?.deductibility || '')}" />
                        </div>
                    </div>
                    <div>
                        <label class="dt-label">Street Address</label>
                        <input id="charity-street" class="dt-input" value="${escapeHtml(existing?.street || '')}" />
                    </div>
                    <div class="grid gap-4 sm:grid-cols-3">
                        <div>
                            <label class="dt-label">City</label>
                            <input id="charity-city" ${!isEditMode ? 'required' : ''} class="dt-input" value="${escapeHtml(existing?.city || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">State</label>
                            <input id="charity-state" ${!isEditMode ? 'required' : ''} class="dt-input" value="${escapeHtml(existing?.state || '')}" />
                        </div>
                        <div>
                            <label class="dt-label">Zip Code</label>
                            <input id="charity-zip" class="dt-input" value="${escapeHtml(existing?.zip || '')}" />
                        </div>
                    </div>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">${isEditMode ? 'Save Changes' : 'Add Charity'}</button>
                    </div>
                </form>
            </div>
            <div class="dt-panel overflow-hidden">
                <div class="hidden overflow-x-auto md:block">
                    <table class="min-w-full divide-y divide-slate-200">
                        <thead class="bg-slate-50">
                            <tr>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Name</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">EIN</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Category</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Status</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Deductibility</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Address</th>
                                <th class="px-5 py-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500">Actions</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 bg-white">
                            ${charities.length === 0 ? '<tr><td colspan="7" class="px-5 py-8 text-sm text-slate-500">No cached charities.</td></tr>' : charities.map(c => `
                                <tr class="hover:bg-slate-50/70">
                                    <td class="px-5 py-3 text-sm font-medium text-slate-900">${escapeHtml(c.name)}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">${escapeHtml(c.ein || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">${escapeHtml(c.category || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">${escapeHtml(c.status || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">${escapeHtml(c.deductibility || '—')}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">${escapeHtml(formatAddress(c))}</td>
                                    <td class="px-5 py-3 text-sm text-slate-700">
                                        <button class="edit-charity-btn text-sm font-medium text-indigo-600 hover:text-indigo-700 mr-3" data-id="${c.id}">Edit</button>
                                        <button class="delete-charity-btn text-sm font-medium text-rose-600 hover:text-rose-700" data-id="${c.id}">Delete</button>
                                    </td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
                <div class="space-y-3 p-4 md:hidden">
                    ${charities.length === 0 ? '<div class="rounded-xl border border-slate-200 bg-white p-4 text-sm text-slate-500">No cached charities.</div>' : charities.map(c => `
                        <article class="rounded-xl border border-slate-200 bg-white p-4">
                            <p class="text-sm font-semibold text-slate-900">${escapeHtml(c.name)}</p>
                            <p class="mt-1 text-xs text-slate-500">${escapeHtml(c.ein || 'No EIN')}</p>
                            <p class="mt-2 text-sm text-slate-600">${escapeHtml(formatAddress(c))}</p>
                            <div class="mt-3 flex gap-2">
                                <button class="edit-charity-btn dt-btn-secondary flex-1" data-id="${c.id}">Edit</button>
                                <button class="delete-charity-btn dt-btn-danger flex-1" data-id="${c.id}">Delete</button>
                            </div>
                        </article>
                    `).join('')}
                </div>
            </div>
        </div>
    `;const norm=(value)=>{if(value===null||value===undefined)return null;const trimmed=String(value).trim();return trimmed?trimmed:null;};const nameInput=document.getElementById('charity-name');const einInput=document.getElementById('charity-ein');const nonprofitTypeInput=document.getElementById('charity-nonprofit-type');const categoryInput=document.getElementById('charity-category');const statusInput=document.getElementById('charity-status');const classificationInput=document.getElementById('charity-classification');const deductibilityInput=document.getElementById('charity-deductibility');const streetInput=document.getElementById('charity-street');const cityInput=document.getElementById('charity-city');const stateInput=document.getElementById('charity-state');const zipInput=document.getElementById('charity-zip');const suggestions=document.getElementById('charity-name-suggestions');let searchTimer=null;if(!isEditMode&&nameInput){nameInput.addEventListener('input',()=>{const query=nameInput.value.trim();if(searchTimer)clearTimeout(searchTimer);if(!query||query.length<2){suggestions.innerHTML='';suggestions.classList.add('hidden');return;}searchTimer=setTimeout(async()=>{try{const{res,data}=await apiJson(`/api/charities/search?q=${encodeURIComponent(query)}`);if(!res.ok||!data||!Array.isArray(data.results)||data.results.length===0){suggestions.innerHTML='';suggestions.classList.add('hidden');return;}suggestions.innerHTML=data.results.slice(0,7).map(item=>`
                        <button type="button" class="charity-suggestion-item w-full border-b border-slate-100 p-2 text-left last:border-b-0 hover:bg-slate-50" data-name="${escapeHtml(item.name || '')}" data-ein="${escapeHtml(item.ein || '')}" data-location="${escapeHtml(item.location || '')}">
                            <div class="text-sm font-medium text-slate-800">${escapeHtml(item.name || '')}</div>
                            <div class="text-xs text-slate-500">${escapeHtml(item.ein || '')}${item.location ? ` • ${escapeHtml(item.location)}` : ''}</div>
                        </button>
                    `).join('');suggestions.classList.remove('hidden');suggestions.querySelectorAll('.charity-suggestion-item').forEach(button=>{button.addEventListener('click',async()=>{const selectedName=button.dataset.name||'';const selectedEin=button.dataset.ein||'';const selectedLocation=button.dataset.location||'';nameInput.value=selectedName;einInput.value=selectedEin;if(selectedLocation&&selectedLocation.includes(',')){const[cityPart,statePart]=selectedLocation.split(',');if(cityPart&&!cityInput.value.trim())cityInput.value=cityPart.trim();if(statePart&&!stateInput.value.trim())stateInput.value=statePart.trim();}suggestions.classList.add('hidden');suggestions.innerHTML='';if(!selectedEin)return;try{const detail=await lookupCharityByEinOnServer(selectedEin);if(!detail)return;if(detail.name)nameInput.value=detail.name;if(detail.ein)einInput.value=detail.ein;if(detail.nonprofit_type)nonprofitTypeInput.value=detail.nonprofit_type;if(detail.category)categoryInput.value=detail.category;if(detail.status)statusInput.value=detail.status;if(detail.classification)classificationInput.value=detail.classification;if(detail.deductibility)deductibilityInput.value=detail.deductibility;if(detail.street)streetInput.value=detail.street;if(detail.city)cityInput.value=detail.city;if(detail.state)stateInput.value=detail.state;if(detail.zip)zipInput.value=detail.zip;}catch(detailErr){console.warn('Charity EIN lookup failed',detailErr);}});});}catch(err){console.error('Charity typeahead failed',err);suggestions.classList.add('hidden');}},300);});}document.getElementById('charity-page-form').addEventListener('submit',async(e)=>{e.preventDefault();const name=nameInput.value.trim();const ein=einInput.value.trim()||'';const category=norm(categoryInput.value);const status=norm(statusInput.value);const classification=norm(classificationInput.value);const nonprofit_type=norm(nonprofitTypeInput.value);const deductibility=norm(deductibilityInput.value);const street=norm(streetInput.value);const city=norm(cityInput.value);const state=norm(stateInput.value);const zip=norm(zipInput.value);if(!name){alert('Name required');return;}if(!isEditMode&&(!city||!state)){alert('City and State are required');return;}try{let charity=null;if(isEditMode&&existing?.id){const updatePayload={name,ein:ein||null,category,status,classification,nonprofit_type,deductibility,street,city,state,zip};const resp=await updateCharityOnServer(existing.id,updatePayload);charity=resp&&resp.charity?resp.charity:{id:existing.id,...updatePayload};}else{const resp=await createOrGetCharityOnServer({name,ein:ein||null,category,status,classification,nonprofit_type,deductibility,street,city,state,zip});charity=resp&&resp.charity?resp.charity:null;if(!charity||!charity.id)throw new Error('Failed to create charity');}if(charity&&userId){await db.charities.put({id:charity.id,user_id:userId,name:charity.name,ein:charity.ein||'',category:charity.category||null,status:charity.status||null,classification:charity.classification||null,nonprofit_type:charity.nonprofit_type||null,deductibility:charity.deductibility||null,street:charity.street||null,city:charity.city||null,state:charity.state||null,zip:charity.zip||null,cached_at:Date.now()});}pendingCharityEditId=null;await updateTotals();await renderCharities();}catch(err){console.error(err);alert(isEditMode?'Failed to update charity':'Failed to add charity');}});const cancelEditBtn=document.getElementById('btn-cancel-charity-edit');if(cancelEditBtn){cancelEditBtn.addEventListener('click',async()=>{pendingCharityEditId=null;await renderCharities();});}document.querySelectorAll('.edit-charity-btn').forEach(button=>{button.addEventListener('click',async(e)=>{pendingCharityEditId=e.currentTarget.dataset.id||null;await renderCharities();});});document.querySelectorAll('.delete-charity-btn').forEach(b=>{b.addEventListener('click',async(e)=>{const charityId=e.currentTarget.dataset.id;if(!confirm('Delete cached charity?'))return;try{const userId=getCurrentUserId();if(userId&&charityId){await deleteCharityOnServer(charityId);await db.charities.delete(charityId);}await renderCharities();}catch(err){console.error(err);alert(err.message||'Failed to delete');}});});}async function renderPersonalInfo(){const root=document.getElementById('route-content')||document.getElementById('app');let profile={name:'',email:'',phone:'',tax_id:'',filing_status:'single',agi:'',marginal_tax_rate:'0.22',itemize_deductions:false};try{const{res,data}=await apiJson('/api/me');if(res.ok&&data){profile={name:data.name||'',email:data.email||'',phone:data.phone||'',tax_id:data.tax_id||'',filing_status:data.filing_status||'single',agi:data.agi??'',marginal_tax_rate:data.marginal_tax_rate??'0.22',itemize_deductions:!!data.itemize_deductions};}}catch(e){console.warn('Failed to load profile from server',e);}root.innerHTML=`
        <div class="mx-auto max-w-3xl space-y-4">
            <div>
                <h1 class="text-2xl font-semibold text-slate-900">Personal Info</h1>
                <p class="mt-1 text-sm text-slate-600">Maintain your profile and tax inputs for more accurate IRS-based savings estimates.</p>
            </div>
            <div class="dt-panel p-6">
                <form id="personal-form" class="space-y-4">
                    <div>
                        <label class="dt-label">Full name</label>
                        <input id="profile-name" type="text" value="${escapeHtml(profile.name)}" class="dt-input" />
                    </div>
                    <div>
                        <label class="dt-label">Email</label>
                        <input id="profile-email" type="email" value="${escapeHtml(profile.email)}" class="dt-input" />
                    </div>
                    <div class="grid grid-cols-2 gap-4">
                        <div>
                            <label class="dt-label">Phone</label>
                            <input id="profile-phone" type="text" value="${escapeHtml(profile.phone)}" class="dt-input" />
                        </div>
                        <div>
                            <label class="dt-label">Tax ID</label>
                            <input id="profile-tax" type="text" value="${escapeHtml(profile.tax_id)}" class="dt-input" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Filing Status</label>
                            <select id="profile-filing-status" class="dt-input">
                                <option value="single" ${profile.filing_status === 'single' ? 'selected' : ''}>Single</option>
                                <option value="married_joint" ${profile.filing_status === 'married_joint' ? 'selected' : ''}>Married filing jointly</option>
                                <option value="married_separate" ${profile.filing_status === 'married_separate' ? 'selected' : ''}>Married filing separately</option>
                                <option value="head_household" ${profile.filing_status === 'head_household' ? 'selected' : ''}>Head of household</option>
                            </select>
                        </div>
                        <div>
                            <label class="dt-label">Adjusted Gross Income (AGI)</label>
                            <input id="profile-agi" type="number" min="0" step="0.01" value="${escapeHtml(profile.agi)}" class="dt-input" />
                        </div>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label class="dt-label">Marginal Tax Rate</label>
                            <input id="profile-marginal-rate" type="number" min="0" max="1" step="0.0001" value="${escapeHtml(profile.marginal_tax_rate)}" class="dt-input" placeholder="0.22 for 22%" />
                        </div>
                        <div class="flex items-end">
                            <label class="inline-flex items-center gap-2 pb-2 text-sm text-slate-700">
                                <input id="profile-itemize" type="checkbox" class="h-4 w-4 rounded border-slate-300" ${profile.itemize_deductions ? 'checked' : ''} />
                                I itemize deductions on Schedule A
                            </label>
                        </div>
                    </div>
                    <p class="text-xs text-slate-500">2026 rule note: non-itemizers may deduct up to $1,000 cash contributions ($2,000 married filing jointly).</p>
                    <div class="flex justify-end">
                        <button type="submit" class="dt-btn-primary">Save</button>
                    </div>
                </form>
            </div>
        </div>
    `;document.getElementById('personal-form').addEventListener('submit',async(e)=>{e.preventDefault();const profile={name:document.getElementById('profile-name').value.trim(),email:document.getElementById('profile-email').value.trim(),phone:document.getElementById('profile-phone').value.trim(),tax_id:document.getElementById('profile-tax').value.trim(),filing_status:document.getElementById('profile-filing-status').value,agi:parseFloat(document.getElementById('profile-agi').value||''),marginal_tax_rate:parseFloat(document.getElementById('profile-marginal-rate').value||''),itemize_deductions:document.getElementById('profile-itemize').checked,};if(!Number.isFinite(profile.agi))profile.agi=null;if(!Number.isFinite(profile.marginal_tax_rate))profile.marginal_tax_rate=null;try{const{res,data}=await apiJson('/api/me',{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify(profile)});if(!res.ok)throw new Error(typeof data==='string'?data:'Failed to save profile');if(data&&data.id){setCurrentUser(data);}alert('Saved');await updateTotals();}catch(err){console.error(err);alert('Failed to save profile');}});}function parseAmount(value){const parsed=parseFloat(value);return Number.isFinite(parsed)?parsed:null;}function normalizeDonationCategory(category){if(!category)return'money';const normalized=String(category).toLowerCase();if(normalized==='items'||normalized==='money'||normalized==='mileage'){return normalized;}return'money';}function emptyFigure(){return{amount:0,count:0,hasAmount:false};}function calculateDonationFigures(donations){const figures={total:emptyFigure(),items:emptyFigure(),money:emptyFigure(),mileage:emptyFigure()};for(const donation of donations||[]){const category=normalizeDonationCategory(donation.category);const bucket=figures[category]||figures.money;const amount=parseAmount(donation.amount);figures.total.count+=1;bucket.count+=1;if(amount!==null){figures.total.amount+=amount;figures.total.hasAmount=true;bucket.amount+=amount;bucket.hasAmount=true;}}return figures;}function formatFigureText(figure){if(!figure)return'$0.00';if(figure.hasAmount)return`$${figure.amount.toFixed(2)}`;if(figure.count>0)return String(figure.count);return'$0.00';}function formatCurrency(amount){return`$${(Number(amount) || 0).toFixed(2)}`;}function isLikelyQualifiedCharity(charity){if(!charity)return false;const deductibility=String(charity.deductibility||'').toLowerCase();if(!deductibility)return true;return deductibility.includes('deductible')||deductibility.includes('pc')||deductibility.includes('public charity');}function normalizeFilingStatus(status){const normalized=String(status||'single').toLowerCase();if(normalized==='married_joint'||normalized==='married_separate'||normalized==='head_household'||normalized==='single'){return normalized;}return'single';}async function calculateTaxEstimates(donations,charities,receipts){const profile=getCurrentUser()||{};const filingStatus=normalizeFilingStatus(profile.filing_status);const itemizeDeductions=!!profile.itemize_deductions;const agi=Number.isFinite(profile.agi)&&profile.agi>0?Number(profile.agi):null;const marginalTaxRate=Number.isFinite(profile.marginal_tax_rate)&&profile.marginal_tax_rate>=0&&profile.marginal_tax_rate<=1?Number(profile.marginal_tax_rate):0.22;const charitiesById=new Map((charities||[]).map(c=>[c.id,c]));const receiptCountByDonation=new Map();for(const receipt of receipts||[]){const donationId=receipt&&receipt.donation_id?String(receipt.donation_id):null;if(!donationId)continue;receiptCountByDonation.set(donationId,(receiptCountByDonation.get(donationId)||0)+1);}const years=new Map();for(const donation of donations||[]){const year=Number(donation.year)||(donation.date?new Date(donation.date).getFullYear():new Date().getFullYear());if(!years.has(year))years.set(year,{cash:[],nonCash:[]});const amount=parseAmount(donation.amount);if(amount===null||amount<=0)continue;const category=normalizeDonationCategory(donation.category);const charity=donation.charity_id?charitiesById.get(donation.charity_id):null;const qualified=isLikelyQualifiedCharity(charity);if(!qualified)continue;const receiptCount=receiptCountByDonation.get(donation.id)||0;const isMonetary=category==='money'||category==='mileage';if(isMonetary&&receiptCount<1)continue;const entry={id:donation.id,amount,category};if(isMonetary){years.get(year).cash.push(entry);}else{years.get(year).nonCash.push(entry);}}const perDonation=new Map();let totalEstimated=0;for(const[year,buckets]of years.entries()){const cashTotal=buckets.cash.reduce((sum,donation)=>sum+donation.amount,0);const nonCashTotal=buckets.nonCash.reduce((sum,donation)=>sum+donation.amount,0);let deductibleCash=0;let deductibleNonCash=0;if(itemizeDeductions){const cashCap=agi?agi*0.60:Number.POSITIVE_INFINITY;const nonCashCap=agi?agi*0.30:Number.POSITIVE_INFINITY;deductibleCash=Math.min(cashTotal,cashCap);deductibleNonCash=Math.min(nonCashTotal,nonCashCap);}else if(year>=2026){const nonItemizerCashCap=filingStatus==='married_joint'?2000:1000;deductibleCash=Math.min(cashTotal,nonItemizerCashCap);deductibleNonCash=0;}const cashRatio=cashTotal>0?deductibleCash/cashTotal:0;const nonCashRatio=nonCashTotal>0?deductibleNonCash/nonCashTotal:0;for(const donation of buckets.cash){const estimated=donation.amount*cashRatio*marginalTaxRate;perDonation.set(donation.id,estimated);totalEstimated+=estimated;}for(const donation of buckets.nonCash){const estimated=donation.amount*nonCashRatio*marginalTaxRate;perDonation.set(donation.id,estimated);totalEstimated+=estimated;}}return{totalEstimated,perDonation};}async function uploadReceiptForDonation(file,donationId){const uploadRes=await fetch('/api/receipts/upload',{method:'POST',headers:{'Content-Type':'application/json'},credentials:'include',body:JSON.stringify({file_type:file.type})});if(!uploadRes.ok)throw new Error('Failed to request upload URL');const uploadData=await uploadRes.json();const putRes=await fetch(uploadData.upload_url,{method:'PUT',headers:{'Content-Type':file.type},body:file});if(!putRes.ok&&putRes.status!==200&&putRes.status!==204){throw new Error('Receipt upload failed');}const confirmRes=await fetch('/api/receipts/confirm',{method:'POST',headers:{'Content-Type':'application/json'},credentials:'include',body:JSON.stringify({key:uploadData.key,file_name:file.name,content_type:file.type,size:file.size,donation_id:donationId})});if(!confirmRes.ok)throw new Error('Failed to confirm receipt');const body=await confirmRes.json();await db.receipts.put({id:crypto.randomUUID(),key:uploadData.key,file_name:file.name,content_type:file.type,size:file.size,donation_id:donationId,uploaded_at:new Date().toISOString(),server_id:body&&body.id?body.id:null});}async function updateTotals(){try{const donations=await getUserDonations();const figures=calculateDonationFigures(donations);const userId=getCurrentUserId();const charities=userId?await db.charities.where('user_id').equals(userId).toArray():[];const receipts=await db.receipts.toArray();const taxEstimates=await calculateTaxEstimates(donations,charities,receipts);const totalEl=document.getElementById('total-donations-amount');const estEl=document.getElementById('estimated-savings');const itemsEl=document.getElementById('items-amount');const moneyEl=document.getElementById('money-amount');const mileageEl=document.getElementById('mileage-amount');const charitiesCountEl=document.getElementById('charities-count');if(totalEl)totalEl.textContent=formatCurrency(figures.total.amount);if(estEl){estEl.textContent=`${formatCurrency(taxEstimates.totalEstimated)} in estimated tax savings`;}if(itemsEl)itemsEl.textContent=formatCurrency(figures.items.amount);if(moneyEl)moneyEl.textContent=formatCurrency(figures.money.amount);if(mileageEl)mileageEl.textContent=formatCurrency(figures.mileage.amount);if(charitiesCountEl){const count=userId?await db.charities.where('user_id').equals(userId).count():0;charitiesCountEl.textContent=`${count} charities`;}}catch(e){console.error('Failed to update totals',e);}}async function updateSyncStatus(){const statusEl=document.getElementById('sync-status');if(!statusEl)return;const isOnline=navigator.onLine;const userId=getCurrentUserId();let pending=0;try{if(userId)pending=await db.sync_queue.where('user_id').equals(userId).count();}catch(e){}const pendingLabel=pending>0?` • ${pending} pending`:'';if(isOnline){statusEl.innerHTML=`<i data-lucide="cloud" class="h-4 w-4 mr-1 text-green-500"></i> Online${pendingLabel}${pending > 0 ? ' <button id="sync-now-btn" class="ml-2 text-xs text-blue-600 underline">Sync now</button>' : ''}`;}else{statusEl.innerHTML=`<i data-lucide="cloud-off" class="h-4 w-4 mr-1 text-red-500"></i> Offline${pendingLabel}`;}if(pending>0&&isOnline){const btn=document.getElementById('sync-now-btn');if(btn)btn.addEventListener('click',()=>Sync.pushChanges());}if(window.lucide)lucide.createIcons();}async function init(){console.log('App initializing...');try{await db.open();}catch(e){const schemaResetKey='dexie_schema_reset_done';const errorName=(e&&e.name)?e.name:'';const message=(e&&e.message)?e.message:String(e);const isSchemaMismatch=message.includes('not indexed')||message.includes('KeyPath')||message.includes('primary key')||errorName==='UpgradeError';if(isSchemaMismatch){const alreadyReset=sessionStorage.getItem(schemaResetKey)==='1';if(alreadyReset){console.error('Dexie schema reset already attempted for this session; aborting retry loop.',e);throw e;}console.warn('Dexie schema mismatch detected. Clearing local database and reloading.');sessionStorage.setItem(schemaResetKey,'1');try{await db.delete();}catch(de){}window.location.reload();return;}sessionStorage.removeItem(schemaResetKey);throw e;}try{sessionStorage.removeItem('dexie_schema_reset_done');}catch(_){}if(window.lucide){lucide.createIcons();}const updateStatus=async()=>{const isOnline=navigator.onLine;if(isOnline){Sync.pushChanges().catch(err=>console.error('Initial sync failed:',err));}await updateSyncStatus();};window.addEventListener('online',updateStatus);window.addEventListener('offline',updateStatus);window.addEventListener('sync-queue-changed',updateSyncStatus);updateStatus();document.querySelectorAll('[data-route]').forEach(a=>{a.addEventListener('click',async(e)=>{e.preventDefault();const link=e.currentTarget;const route=link?link.dataset.route:null;const mobile=document.getElementById('mobile-menu');if(mobile&&!mobile.classList.contains('hidden'))mobile.classList.add('hidden');try{const isAuthed=await checkAuthCached();if(!isAuthed){AUTHENTICATED=false;RETURN_TO=route;renderLogin();return;}AUTHENTICATED=true;if(route)await navigate(route);}catch(err){console.warn('Auth check failed',err);AUTHENTICATED=false;RETURN_TO=route;renderLogin();}});});const mobileButton=document.getElementById('mobile-menu-button');if(mobileButton){mobileButton.addEventListener('click',()=>{const mobile=document.getElementById('mobile-menu');if(!mobile)return;mobile.classList.toggle('hidden');});}const btnAddDonation=document.getElementById('btn-add-donation');if(btnAddDonation){btnAddDonation.addEventListener('click',()=>openAddDonationModal());}const btnAddDonationMobile=document.getElementById('btn-add-donation-mobile');if(btnAddDonationMobile){btnAddDonationMobile.addEventListener('click',async()=>{const mobile=document.getElementById('mobile-menu');if(mobile)mobile.classList.add('hidden');await openAddDonationModal();});}const btnAddDonationTop=document.getElementById('btn-add-donation-top');if(btnAddDonationTop){btnAddDonationTop.addEventListener('click',()=>openAddDonationModal());}const btnItemsAdd=document.getElementById('btn-items-add');if(btnItemsAdd)btnItemsAdd.addEventListener('click',()=>openAddDonationModal('items'));const btnMoneyAdd=document.getElementById('btn-money-add');if(btnMoneyAdd)btnMoneyAdd.addEventListener('click',()=>openAddDonationModal('money'));const btnMileageAdd=document.getElementById('btn-mileage-add');if(btnMileageAdd)btnMileageAdd.addEventListener('click',()=>openAddDonationModal('mileage'));const btnItemsView=document.getElementById('btn-items-view');if(btnItemsView)btnItemsView.addEventListener('click',async()=>navigate('/donations'));const btnMoneyView=document.getElementById('btn-money-view');if(btnMoneyView)btnMoneyView.addEventListener('click',async()=>navigate('/donations'));const btnMileageView=document.getElementById('btn-mileage-view');if(btnMileageView)btnMileageView.addEventListener('click',async()=>navigate('/donations'));const btnCharityAddDashboard=document.getElementById('btn-charity-add-dashboard');if(btnCharityAddDashboard)btnCharityAddDashboard.addEventListener('click',()=>openAddCharityModal());const btnCharityViewDashboard=document.getElementById('btn-charity-view-dashboard');if(btnCharityViewDashboard)btnCharityViewDashboard.addEventListener('click',async()=>navigate('/charities'));const btnUploadReceiptMobile=document.getElementById('btn-upload-receipt-mobile');if(btnUploadReceiptMobile){btnUploadReceiptMobile.addEventListener('click',()=>{const mobile=document.getElementById('mobile-menu');if(mobile)mobile.classList.add('hidden');const desktopUploadBtn=document.getElementById('btn-upload-receipt');if(desktopUploadBtn)desktopUploadBtn.click();});}const btnLogout=document.getElementById('btn-logout');if(btnLogout){btnLogout.addEventListener('click',handleLogout);}const btnLogoutMobile=document.getElementById('btn-logout-mobile');if(btnLogoutMobile){btnLogoutMobile.addEventListener('click',async()=>{const mobile=document.getElementById('mobile-menu');if(mobile)mobile.classList.add('hidden');await handleLogout();});}window.addEventListener('popstate',async(e)=>{const path=location.pathname;try{const isAuthed=await checkAuthCached();if(!isAuthed){AUTHENTICATED=false;RETURN_TO=path;window.history.replaceState({},'','/');renderLogin();return;}AUTHENTICATED=true;await navigate(path,{pushState:false});}catch(err){console.warn('Auth check failed on popstate',err);AUTHENTICATED=false;RETURN_TO=path;window.history.replaceState({},'','/');renderLogin();}});try{console.log('Seeding database...');await seedDatabase();await updateTotals();const isAuthed=await checkAuthCached();if(!isAuthed){console.log('Not authenticated, rendering login');AUTHENTICATED=false;document.getElementById('nav-container').classList.add('hidden');document.getElementById('auth-actions').classList.add('hidden');updateHomeSummaryVisibility('/login');await clearUserCaches();clearCurrentUser();const requestedPath=location.pathname;if(requestedPath!=='/'&&requestedPath!=='/index.html'){RETURN_TO=requestedPath;window.history.replaceState({},'','/');}renderLogin();}else{console.log('Authenticated, navigating to route');AUTHENTICATED=true;if(!getCurrentUserId()){try{const res=await fetch('/api/me',{credentials:'include'});if(res.ok){const profile=await res.json();setCurrentUser(profile);}}catch(e){}}try{try{await fetch('/api/valuations/seed',{method:'POST',credentials:'include'});}catch(e){}await refreshCharitiesCache();await refreshDonationsFromServer();}catch(e){}document.getElementById('nav-container').classList.remove('hidden','sm:hidden');document.getElementById('auth-actions').classList.remove('hidden');const initialRoute=location.pathname==='/index.html'||location.pathname==='/'?'/':location.pathname;await navigate(initialRoute);}}catch(err){console.error('Initialization failed:',err);const safeMessage=escapeHtml(err&&err.message?err.message:'Unknown error');document.getElementById('app').innerHTML=`
            <div class="p-8 text-center">
                <h1 class="text-rose-600 font-semibold">Initialization Error</h1>
                <p class="text-slate-600">${safeMessage}</p>
                <button onclick="location.reload()" class="dt-btn-primary mt-4">Retry</button>
            </div>
        `;}}if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',init);}else{init();}