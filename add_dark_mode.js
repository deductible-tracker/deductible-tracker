const fs = require('fs');
const path = require('path');

function walk(dir, callback) {
    fs.readdirSync(dir).forEach(f => {
        let dirPath = path.join(dir, f);
        let isDirectory = fs.statSync(dirPath).isDirectory();
        isDirectory ? walk(dirPath, callback) : callback(path.join(dir, f));
    });
}

const replacements = [
    { regex: /bg-white(?! dark:)/g, replacement: 'bg-white dark:bg-slate-800' },
    { regex: /text-slate-900(?! dark:)/g, replacement: 'text-slate-900 dark:text-slate-100' },
    { regex: /text-slate-600(?! dark:)/g, replacement: 'text-slate-600 dark:text-slate-300' },
    { regex: /text-slate-500(?! dark:)/g, replacement: 'text-slate-500 dark:text-slate-400' },
    { regex: /border-slate-200(?! dark:)/g, replacement: 'border-slate-200 dark:border-slate-700' },
    { regex: /border-slate-100(?! dark:)/g, replacement: 'border-slate-100 dark:border-slate-700' },
    { regex: /bg-slate-50(?! dark:)/g, replacement: 'bg-slate-50 dark:bg-slate-700\/50' },
    { regex: /hover:bg-slate-50(?! dark:)/g, replacement: 'hover:bg-slate-50 dark:hover:bg-slate-700' },
    { regex: /hover:bg-slate-100(?! dark:)/g, replacement: 'hover:bg-slate-100 dark:hover:bg-slate-700' },
    { regex: /divide-slate-100(?! dark:)/g, replacement: 'divide-slate-100 dark:divide-slate-700' },
    { regex: /divide-slate-200(?! dark:)/g, replacement: 'divide-slate-200 dark:divide-slate-700' },
    { regex: /text-indigo-600(?! dark:)/g, replacement: 'text-indigo-600 dark:text-indigo-400' },
    { regex: /text-indigo-700(?! dark:)/g, replacement: 'text-indigo-700 dark:text-indigo-300' },
    { regex: /bg-indigo-600(?! dark:)/g, replacement: 'bg-indigo-600 dark:bg-indigo-500' },
    { regex: /hover:bg-indigo-700(?! dark:)/g, replacement: 'hover:bg-indigo-700 dark:hover:bg-indigo-600' },
    { regex: /text-emerald-600(?! dark:)/g, replacement: 'text-emerald-600 dark:text-emerald-400' },
    { regex: /text-rose-600(?! dark:)/g, replacement: 'text-rose-600 dark:text-rose-400' },
    { regex: /bg-rose-100(?! dark:)/g, replacement: 'bg-rose-100 dark:bg-rose-900\/30' },
    { regex: /text-rose-700(?! dark:)/g, replacement: 'text-rose-700 dark:text-rose-300' },
    { regex: /bg-emerald-100(?! dark:)/g, replacement: 'bg-emerald-100 dark:bg-emerald-900\/30' },
    { regex: /text-emerald-700(?! dark:)/g, replacement: 'text-emerald-700 dark:text-emerald-300' },
    { regex: /bg-slate-100(?! dark:)/g, replacement: 'bg-slate-100 dark:bg-slate-800' },
    { regex: /text-slate-700(?! dark:)/g, replacement: 'text-slate-700 dark:text-slate-300' },
];

walk('./static/js', function(filePath) {
    if (filePath.endsWith('.js')) {
        let content = fs.readFileSync(filePath, 'utf8');
        let original = content;
        replacements.forEach(r => {
            content = content.replace(r.regex, r.replacement);
        });
        if (content !== original) {
            fs.writeFileSync(filePath, content, 'utf8');
            console.log('Updated', filePath);
        }
    }
});