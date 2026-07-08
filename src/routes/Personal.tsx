import React, { useState, useEffect } from "react";
import { getCurrentUser, setCurrentUser } from "../services/currentUser";
import { apiJson } from "../services/apiClient";
import { registerVaultKey, isWebAuthnSupported } from "../services/crypto";
import { Sync } from "../services/sync";
import { Shield, ShieldAlert, Download, Upload, Trash2, Key } from "lucide-react";

export default function Personal() {
  const [profile, setProfile] = useState<any>(getCurrentUser() || {});
  const [filingStatus, setFilingStatus] = useState(profile.filing_status || "single");
  const [agi, setAgi] = useState<string>(profile.agi !== null && profile.agi !== undefined ? profile.agi.toString() : "");
  const [marginalRate, setMarginalRate] = useState<string>(
    profile.marginal_tax_rate !== null && profile.marginal_tax_rate !== undefined
      ? (profile.marginal_tax_rate * 100).toString()
      : ""
  );
  const [itemize, setItemize] = useState(!!profile.itemize_deductions);
  const [vaultEnabled, setVaultEnabled] = useState(!!profile.is_encrypted);
  const [message, setMessage] = useState("");
  const [errMessage, setErrMessage] = useState("");
  const [loading, setLoading] = useState(false);
  const [restoreFile, setRestoreFile] = useState<File | null>(null);

  useEffect(() => {
    // Whenever filingStatus or AGI changes, query the marginal rate API to autofill
    const fetchMarginalRate = async () => {
      const parsedAgi = parseFloat(agi);
      if (isNaN(parsedAgi) || parsedAgi < 0) return;

      try {
        const { res, data } = await apiJson(
          `/api/tax/marginal-rate?filing_status=${filingStatus}&agi=${parsedAgi}`
        );
        if (res.ok && data.selected_rate !== null) {
          setMarginalRate((data.selected_rate * 100).toFixed(1));
        }
      } catch (err) {
        console.error("Autofill marginal rate error:", err);
      }
    };

    const timer = setTimeout(fetchMarginalRate, 500);
    return () => clearTimeout(timer);
  }, [filingStatus, agi]);

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    setMessage("");
    setErrMessage("");
    setLoading(true);

    const parsedAgi = parseFloat(agi);
    const parsedRate = parseFloat(marginalRate);

    const payload = {
      filing_status: filingStatus,
      agi: isNaN(parsedAgi) ? null : parsedAgi,
      marginal_tax_rate: isNaN(parsedRate) ? null : parsedRate / 100,
      itemize_deductions: itemize,
    };

    try {
      const { res, data } = await apiJson("/api/me", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (res.ok) {
        setCurrentUser(data);
        setProfile(data);
        setMessage("Profile updated successfully.");
        // Queue sync action
        await Sync.queueProfileUpdate(data.id, data);
      } else {
        setErrMessage("Failed to save profile details.");
      }
    } catch (err) {
      setErrMessage("Connection error.");
    } finally {
      setLoading(false);
    }
  };

  const handleEnableVault = async () => {
    setMessage("");
    setErrMessage("");

    if (!isWebAuthnSupported()) {
      setErrMessage("WebAuthn / Passkeys not supported in this browser.");
      return;
    }

    try {
      setLoading(true);
      const { key, credentialId } = await registerVaultKey(profile.id);

      const payload = {
        is_encrypted: true,
        vault_credential_id: credentialId,
      };

      const { res, data } = await apiJson("/api/me", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (res.ok) {
        setCurrentUser(data);
        setProfile(data);
        setVaultEnabled(true);
        setMessage("Local passkey vault enabled! Your sensitive data is now encrypted.");
        await Sync.queueProfileUpdate(data.id, data);
      } else {
        setErrMessage("Failed to configure vault on server.");
      }
    } catch (err: any) {
      console.error(err);
      setErrMessage(err.message || "Failed to register passkey vault key.");
    } finally {
      setLoading(false);
    }
  };

  const handleRestoreBackup = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!restoreFile) return;

    setMessage("");
    setErrMessage("");
    setLoading(true);

    const formData = new FormData();
    formData.append("file", restoreFile);

    try {
      const res = await fetch("/api/me/import", {
        method: "POST",
        body: formData,
        headers: {
          // fetch boundary is set automatically
          "X-CSRF-Token": document.cookie.split("; ").find(r => r.startsWith("csrf_token="))?.split("=")[1] || ""
        }
      });

      if (res.ok) {
        setMessage("Backup restored successfully! Page will reload.");
        setTimeout(() => window.location.reload(), 1500);
      } else {
        setErrMessage(await res.text() || "Failed to restore backup.");
      }
    } catch (err) {
      setErrMessage("Error uploading backup zip.");
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteAccount = async () => {
    if (
      !confirm(
        "WARNING: This will permanently delete your account, all donation records, and receipt files. This action CANNOT be undone. Proceed?"
      )
    ) {
      return;
    }

    setLoading(true);
    try {
      const { res } = await apiJson("/api/me", { method: "DELETE" });
      if (res.ok) {
        window.location.href = "/login";
      } else {
        setErrMessage("Failed to delete account.");
      }
    } catch (err) {
      setErrMessage("Connection error.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8 max-w-4xl mx-auto">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white font-display">
          Personal Profile & Security
        </h1>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Manage your tax profile settings, zero-knowledge encryption vault, and backup states.
        </p>
      </div>

      {message && (
        <div className="rounded-lg bg-green-50 dark:bg-green-950/20 p-4 border border-green-200 dark:border-green-900/50">
          <p className="text-sm text-green-700 dark:text-green-400 font-medium">{message}</p>
        </div>
      )}
      {errMessage && (
        <div className="rounded-lg bg-rose-50 dark:bg-rose-950/20 p-4 border border-rose-200 dark:border-rose-900/50">
          <p className="text-sm text-rose-700 dark:text-rose-400 font-medium">{errMessage}</p>
        </div>
      )}

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        {/* Tax Profile Form */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm space-y-4">
          <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display">
            Tax Profile Settings
          </h2>
          <form onSubmit={handleSaveProfile} className="space-y-4">
            <div>
              <label className="dt-label">Filing Status</label>
              <select
                value={filingStatus}
                onChange={(e) => setFilingStatus(e.target.value)}
                className="dt-input"
              >
                <option value="single">Single</option>
                <option value="married_joint">Married Filing Jointly</option>
                <option value="married_separate">Married Filing Separately</option>
                <option value="head_household">Head of Household</option>
              </select>
            </div>
            <div>
              <label className="dt-label">Adjusted Gross Income (AGI)</label>
              <input
                type="number"
                value={agi}
                onChange={(e) => setAgi(e.target.value)}
                placeholder="e.g. 75000"
                className="dt-input"
              />
            </div>
            <div>
              <label className="dt-label">Marginal Tax Rate (%)</label>
              <input
                type="number"
                step="0.1"
                value={marginalRate}
                onChange={(e) => setMarginalRate(e.target.value)}
                placeholder="e.g. 22"
                className="dt-input"
              />
            </div>
            <div className="flex items-center gap-2 pt-2">
              <input
                type="checkbox"
                id="itemize"
                checked={itemize}
                onChange={(e) => setItemize(e.target.checked)}
                className="rounded border-slate-300 text-indigo-600 focus:ring-indigo-500 h-4 w-4"
              />
              <label htmlFor="itemize" className="text-sm font-medium text-slate-700 dark:text-slate-300">
                Itemize Tax Deductions
              </label>
            </div>
            <button
              type="submit"
              disabled={loading}
              className="w-full dt-btn-primary py-2 mt-4"
            >
              Save Profile
            </button>
          </form>
        </section>

        {/* Security Vault Section */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm flex flex-col justify-between">
          <div>
            <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display flex items-center gap-2">
              <Shield className="text-indigo-500" size={20} />
              Zero-Knowledge Vault
            </h2>
            <p className="mt-3 text-sm text-slate-600 dark:text-slate-400">
              Protect your privacy. Encrypt your amounts, notes, and charity names in your browser using AES-GCM before syncing to the cloud. Only your local passkey can decrypt it.
            </p>
            {vaultEnabled ? (
              <div className="mt-4 flex items-center gap-2 text-sm text-green-600 dark:text-green-400 font-semibold bg-green-500/10 p-3 rounded-lg border border-green-500/20">
                <Key size={16} /> Encryption Vault Active
              </div>
            ) : (
              <div className="mt-4 flex items-center gap-2 text-sm text-yellow-600 dark:text-yellow-400 font-semibold bg-yellow-500/10 p-3 rounded-lg border border-yellow-500/20">
                <ShieldAlert size={16} /> Encryption Vault Disabled
              </div>
            )}
          </div>
          {!vaultEnabled && (
            <button
              onClick={handleEnableVault}
              disabled={loading}
              className="w-full dt-btn-primary py-2 mt-6 flex items-center justify-center gap-2"
            >
              <Key size={16} /> Enable Passkey Vault
            </button>
          )}
        </section>

        {/* Backup & Restore Data */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm space-y-4">
          <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display flex items-center gap-2">
            Data Portability
          </h2>
          <p className="text-sm text-slate-600 dark:text-slate-400">
            Export a full ZIP copy of your database logs and physical receipt files, or restore from a previous backup.
          </p>
          <div className="space-y-4 pt-2">
            <a
              href="/api/me/export"
              download
              className="flex w-full items-center justify-between gap-3 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-3 text-sm font-medium text-slate-700 dark:text-slate-300 shadow-sm hover:bg-slate-50 dark:hover:bg-slate-700 transition"
            >
              <span className="flex items-center gap-2">
                <Download size={16} className="text-indigo-500" /> Export Backup (.ZIP)
              </span>
              <span>Download</span>
            </a>

            <form onSubmit={handleRestoreBackup} className="space-y-2">
              <label className="dt-label">Restore from Backup ZIP</label>
              <div className="flex gap-2">
                <input
                  type="file"
                  accept=".zip"
                  required
                  onChange={(e) => setRestoreFile(e.target.files?.[0] || null)}
                  className="dt-input mt-0 text-xs py-1.5"
                />
                <button
                  type="submit"
                  disabled={loading || !restoreFile}
                  className="dt-btn-secondary py-1.5 px-3 flex items-center gap-1"
                >
                  <Upload size={14} /> Restore
                </button>
              </div>
            </form>
          </div>
        </section>

        {/* Delete Account */}
        <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm flex flex-col justify-between">
          <div>
            <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display text-rose-600 flex items-center gap-2">
              Danger Zone
            </h2>
            <p className="mt-3 text-sm text-slate-600 dark:text-slate-400">
              Permanently delete your account, settings, and purge all receipts from secure object storage. This process is instant and irreversible.
            </p>
          </div>
          <button
            onClick={handleDeleteAccount}
            disabled={loading}
            className="w-full dt-btn-danger py-2 mt-6 flex items-center justify-center gap-2"
          >
            <Trash2 size={16} /> Delete Account Permanently
          </button>
        </section>
      </div>
    </div>
  );
}
