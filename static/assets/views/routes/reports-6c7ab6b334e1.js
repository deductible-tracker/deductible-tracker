export async function renderReportsRoute(deps){const{apiJson}=deps;const root=document.getElementById('route-content')||document.getElementById('app');let years=[];try{const{res,data}=await apiJson('/api/reports/years');if(res.ok&&data&&Array.isArray(data.years)){years=data.years.filter(y=>Number.isInteger(y)).sort((a,b)=>b-a);}}catch(e){console.warn('Failed to load report years',e);}const hasDonationYears=years.length>0;const currentYear=new Date().getFullYear();if(years.length===0)years=[currentYear];const optionsHtml=years.map(y=>`<option value="${y}">${y}</option>`).join('');root.innerHTML=`
        <div class="mx-auto max-w-7xl space-y-4">
            <div class="flex items-end justify-between gap-4">
                <div>
                    <h1 class="text-2xl font-semibold text-slate-900 dark:text-slate-100">Reports</h1>
                    <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">Generate donation exports for your tax records.</p>
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
                <p class="text-sm text-slate-600 dark:text-slate-300">Select a donation year and export donations as CSV or TXF.</p>
            </div>
        </div>
    `;const yearEl=document.getElementById('export-year');const csvBtn=document.getElementById('btn-export-csv');const taxTxfBtn=document.getElementById('btn-export-tax-txf');if(!hasDonationYears){csvBtn.disabled=true;taxTxfBtn.disabled=true;}const downloadReport=async(endpoint,extension)=>{const year=yearEl.value;try{const res=await fetch(`${endpoint}?year=${encodeURIComponent(year)}`,{credentials:'include'});if(!res.ok)throw new Error('Export failed');const blob=await res.blob();const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=`donations-${year}.${extension}`;document.body.appendChild(a);a.click();a.remove();URL.revokeObjectURL(url);}catch(e){console.error(e);alert('Export failed');}};csvBtn.addEventListener('click',async()=>{await downloadReport('/api/reports/export','csv');});taxTxfBtn.addEventListener('click',async()=>{await downloadReport('/api/reports/export/txf','txf');});}