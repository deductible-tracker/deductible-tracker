import React, { useState, useEffect } from "react";
import { apiJson } from "../services/apiClient";
import { Download, FileSpreadsheet, History } from "lucide-react";

export default function Reports() {
  const [years, setYears] = useState<number[]>([]);
  const [selectedYear, setSelectedYear] = useState<string>("all");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const fetchYears = async () => {
      try {
        const { res, data } = await apiJson("/api/reports/years");
        if (res.ok && data.years) {
          setYears(data.years);
        }
      } catch (err) {
        console.error("Failed to load available report years:", err);
      }
    };
    fetchYears();
  }, []);

  const getExportUrl = (type: "csv" | "txf" | "audit") => {
    let base = "/api/reports/export";
    if (type === "txf") base = "/api/reports/export/txf";
    if (type === "audit") base = "/api/reports/audit";

    const params = new URLSearchParams();
    if (selectedYear !== "all" && type !== "audit") {
      params.append("year", selectedYear);
    }
    return params.toString() ? `${base}?${params.toString()}` : base;
  };

  return (
    <div className="space-y-8 max-w-4xl mx-auto">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white font-display">
          Tax Reporting & Export
        </h1>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Generate IRS-compliant tax files and spreadsheet sheets of your donations.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        {/* Export Form */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm space-y-6">
          <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display">
            Export Parameters
          </h2>
          <div>
            <label className="dt-label">Filing Year</label>
            <select
              value={selectedYear}
              onChange={(e) => setSelectedYear(e.target.value)}
              className="dt-input"
            >
              <option value="all">All Years</option>
              {years.map((y) => (
                <option key={y} value={y.toString()}>
                  {y}
                </option>
              ))}
            </select>
            <p className="mt-2 text-xs text-slate-500">
              Filter exports to a specific tax year or export your entire giving history.
            </p>
          </div>

          <div className="space-y-3 pt-2">
            <a
              href={getExportUrl("csv")}
              download
              className="flex w-full items-center justify-between gap-3 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 text-sm font-medium text-slate-700 dark:text-slate-300 shadow-sm hover:bg-slate-50 dark:hover:bg-slate-700 transition"
            >
              <span className="flex items-center gap-2">
                <FileSpreadsheet className="text-indigo-500" size={18} />
                Export CSV Spreadsheet
              </span>
              <Download size={16} />
            </a>

            <a
              href={getExportUrl("txf")}
              download
              className="flex w-full items-center justify-between gap-3 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 text-sm font-medium text-slate-700 dark:text-slate-300 shadow-sm hover:bg-slate-50 dark:hover:bg-slate-700 transition"
            >
              <span className="flex items-center gap-2">
                <FileSpreadsheet className="text-emerald-500" size={18} />
                Export Tax Exchange Format (.TXF)
              </span>
              <Download size={16} />
            </a>
          </div>
        </section>

        {/* Audit Log / CPA Section */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm space-y-6">
          <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display">
            CPA & Audit Readiness
          </h2>
          <p className="text-sm text-slate-600 dark:text-slate-400">
            Deductible Tracker logs revision history and modifications to provide a clear audit trail for tax professionals.
          </p>
          <div className="rounded-lg bg-slate-50 dark:bg-slate-800/50 p-4 border border-slate-100 dark:border-slate-800 text-xs text-slate-500 space-y-2">
            <p className="font-semibold text-slate-700 dark:text-slate-300">IRS Compliance Note:</p>
            <p>
              Monetary donations require bank records or written communications from the charity. Receipts must include the organization name, date, and amount. Deductible Tracker flags missing receipt criteria automatically.
            </p>
          </div>
          <div className="pt-2">
            <a
              href={getExportUrl("audit")}
              download
              className="flex w-full items-center justify-between gap-3 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 text-sm font-medium text-slate-700 dark:text-slate-300 shadow-sm hover:bg-slate-50 dark:hover:bg-slate-700 transition"
            >
              <span className="flex items-center gap-2">
                <History className="text-slate-500" size={18} />
                Download CPA Audit Logs
              </span>
              <Download size={16} />
            </a>
          </div>
        </section>
      </div>
    </div>
  );
}
