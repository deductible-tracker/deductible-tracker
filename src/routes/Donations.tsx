import React, { useState, useEffect } from "react";
import { useSearchParams } from "react-router";
import db from "../services/db";
import { getCurrentUserId } from "../services/currentUser";
import { Sync } from "../services/sync";
import { apiJson } from "../services/apiClient";
import { ensureVaultKey, decryptBinaryData } from "../services/crypto";
import { attachReceiptFileToDonation, isImageReceipt } from "../services/receiptUpload";
import { formatCurrency } from "../services/donationFigures";
import {
  Search,
  Plus,
  Trash2,
  Edit3,
  Eye,
  X,
  FileText,
  Upload,
  Sparkles,
  ExternalLink,
  ChevronDown,
  Info,
  Calendar,
  DollarSign
} from "lucide-react";

export default function Donations() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [donations, setDonations] = useState<any[]>([]);
  const [charities, setCharities] = useState<any[]>([]);
  const [receipts, setReceipts] = useState<any[]>([]);
  const [years, setYears] = useState<number[]>([]);
  
  // Filters
  const [filterYear, setFilterYear] = useState<string>("all");
  const [filterCategory, setFilterCategory] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState("");

  // Modals / Detail triggers
  const [showAddModal, setShowAddModal] = useState(false);
  const [selectedDonation, setSelectedDonation] = useState<any>(null);
  const [isEditing, setIsEditing] = useState(false);

  // Form states
  const [donDate, setDonDate] = useState("");
  const [donCharityId, setDonCharityId] = useState("");
  const [donCategory, setDonCategory] = useState("money");
  const [donAmount, setDonAmount] = useState("");
  const [donNotes, setDonNotes] = useState("");
  
  // Valuations inside Form
  const [valQuery, setValQuery] = useState("");
  const [valSuggestions, setValSuggestions] = useState<any[]>([]);
  const [itemName, setItemName] = useState("");

  // Receipt upload inside Form
  const [uploadingReceipt, setUploadingReceipt] = useState(false);
  const [ocrSuggestion, setOcrSuggestion] = useState<any>(null);
  const [receiptKey, setReceiptKey] = useState("");
  const [receiptName, setReceiptName] = useState("");
  const [receiptType, setReceiptType] = useState("");
  const [receiptSize, setReceiptSize] = useState<number>(0);
  const [error, setError] = useState("");

  // Receipt viewing inside details
  const [downloadingReceiptId, setDownloadingReceiptId] = useState<string | null>(null);

  const userId = getCurrentUserId();

  const loadData = async () => {
    if (!userId) return;
    const localDons = await db.donations.where("user_id").equals(userId).toArray();
    const activeDons = localDons.filter((d: any) => !d.deleted);
    const localChars = await db.charities.where("user_id").equals(userId).toArray();
    const localReceipts = await db.receipts.toArray();

    // Sort by date desc
    activeDons.sort((a: any, b: any) => new Date(b.date).getTime() - new Date(a.date).getTime());

    setDonations(activeDons);
    setCharities(localChars);
    setReceipts(localReceipts);

    // Extract unique years
    const uniqueYears = Array.from(
      new Set(
        activeDons.map((d: any) =>
          d.year || (d.date ? new Date(d.date).getFullYear() : new Date().getFullYear())
        )
      )
    ).sort((a: any, b: any) => b - a) as number[];
    setYears(uniqueYears);
  };

  useEffect(() => {
    loadData();
    window.addEventListener("sync-queue-changed", loadData);
    return () => {
      window.removeEventListener("sync-queue-changed", loadData);
    };
  }, [userId]);

  // Handle URL navigation actions like ?new=true or ?view=id
  useEffect(() => {
    const isNew = searchParams.get("new") === "true";
    const viewId = searchParams.get("view");

    if (isNew) {
      setShowAddModal(true);
      // clean URL query
      setSearchParams({});
    } else if (viewId) {
      const don = donations.find((d) => d.id === viewId);
      if (don) {
        setSelectedDonation(don);
      }
      setSearchParams({});
    }
  }, [searchParams, donations]);

  const handleValuationSuggest = async (val: string) => {
    setValQuery(val);
    if (!val.trim()) {
      setValSuggestions([]);
      return;
    }
    try {
      const { res, data } = await apiJson("/api/valuations/suggest", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: val }),
      });
      if (res.ok) {
        setValSuggestions(data);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleSelectValuation = (sugg: any) => {
    setItemName(sugg[0]);
    // Autofill average suggested value
    const avg = Math.round((sugg[1] + sugg[2]) / 2);
    setDonAmount(avg.toString());
    setValSuggestions([]);
    setValQuery("");
    setDonNotes((prev) => `${prev ? prev + "\n" : ""}Item: ${sugg[0]} (Value range: $${sugg[1]} - $${sugg[2]})`);
  };

  const handleReceiptUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setError("");
    setUploadingReceipt(true);
    setOcrSuggestion(null);

    try {
      // 1. Upload receipt to storage (handles local AES-GCM encryption if passkey vault active)
      const uploaded = await attachReceiptFileToDonation(file, "temp-donation-id");
      
      setReceiptKey(uploaded.uploaded.key);
      setReceiptName(file.name);
      setReceiptType(file.type);
      setReceiptSize(uploaded.uploaded.size);

      if (uploaded.analysis && uploaded.analysis.status === "done") {
        setOcrSuggestion(uploaded.analysis.suggestion);
      }
    } catch (err: any) {
      setError(err.message || "Failed to upload and analyze receipt.");
    } finally {
      setUploadingReceipt(false);
    }
  };

  const applyOcrSuggestion = () => {
    if (!ocrSuggestion) return;
    if (ocrSuggestion.dateOfDonation) setDonDate(ocrSuggestion.dateOfDonation);
    if (ocrSuggestion.amountUsd) setDonAmount(ocrSuggestion.amountUsd.toString());
    if (ocrSuggestion.donationType) setDonCategory(ocrSuggestion.donationType === "item" ? "items" : "money");
    if (ocrSuggestion.itemName) setItemName(ocrSuggestion.itemName);

    // Try to match charity by name, otherwise add note
    if (ocrSuggestion.organizationName) {
      const match = charities.find(
        (c) => c.name.toLowerCase().includes(ocrSuggestion.organizationName.toLowerCase())
      );
      if (match) {
        setDonCharityId(match.id);
      } else {
        setDonNotes(
          (prev) =>
            `${prev ? prev + "\n" : ""}Suggested Charity: ${ocrSuggestion.organizationName}`
        );
      }
    }
    setOcrSuggestion(null);
  };

  const handleDonationSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (!donDate) {
      setError("Donation date is required.");
      return;
    }
    if (!donCharityId) {
      setError("Please select a charity.");
      return;
    }

    const parsedAmount = parseFloat(donAmount);
    const donationId = isEditing ? selectedDonation.id : crypto.randomUUID();

    const donationData = {
      id: donationId,
      user_id: userId,
      date: donDate,
      year: new Date(donDate).getFullYear(),
      category: donCategory,
      amount: isNaN(parsedAmount) ? 0 : parsedAmount,
      charity_id: donCharityId,
      notes: donNotes.trim() || null,
      updated_at: new Date().toISOString(),
    };

    try {
      // 1. Queue/save donation locally
      await Sync.queueAction("donations", donationData, isEditing ? "update" : "create");

      // 2. Attach uploaded receipt details if present
      if (receiptKey) {
        const receiptData = {
          id: crypto.randomUUID(),
          donation_id: donationId,
          key: receiptKey,
          file_name: receiptName,
          content_type: receiptType,
          size: receiptSize,
          is_encrypted: getCurrentUser()?.is_encrypted || false,
        };
        await Sync.queueAction("receipts", receiptData, "create");
      }

      await loadData();
      closeForm();
    } catch (err: any) {
      setError(err.message || "Failed to save donation.");
    }
  };

  const handleDeleteDonation = async (id: string) => {
    if (!confirm("Are you sure you want to delete this donation record?")) return;

    try {
      await Sync.queueAction("donations", { id }, "delete");
      await loadData();
      setSelectedDonation(null);
    } catch (e: any) {
      alert(e.message || "Failed to delete donation.");
    }
  };

  const handleDownloadReceipt = async (receipt: any) => {
    setDownloadingReceiptId(receipt.id);
    try {
      // 1. Get presigned read URL from Hono
      const { res, data } = await apiJson("/api/receipts/presign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key: receipt.key }),
      });

      if (!res.ok) throw new Error("Failed to get download URL");

      // 2. Fetch binary file contents
      const fileResp = await fetch(data.download_url);
      if (!fileResp.ok) throw new Error("Failed to fetch file contents");

      let fileBuffer = await fileResp.arrayBuffer();

      // 3. Decrypt on-the-fly in browser if encrypted
      if (receipt.is_encrypted) {
        const vaultKey = await ensureVaultKey(userId || "");
        if (vaultKey) {
          fileBuffer = await decryptBinaryData(vaultKey, new Uint8Array(fileBuffer));
        } else {
          throw new Error("Passkey vault is required to decrypt this receipt.");
        }
      }

      // 4. Trigger browser download dialog
      const blob = new Blob([fileBuffer], { type: receipt.content_type || "application/octet-stream" });
      const objectUrl = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = objectUrl;
      link.download = receipt.file_name || `receipt-${receipt.id}`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(objectUrl);
    } catch (err: any) {
      alert(err.message || "Failed to download receipt.");
    } finally {
      setDownloadingReceiptId(null);
    }
  };

  const openEdit = (don: any) => {
    setSelectedDonation(don);
    setIsEditing(true);
    setDonDate(don.date || "");
    setDonCharityId(don.charity_id || "");
    setDonCategory(don.category || "money");
    setDonAmount(don.amount !== null && don.amount !== undefined ? don.amount.toString() : "");
    setDonNotes(don.notes || "");
    setReceiptKey("");
    setReceiptName("");
    setOcrSuggestion(null);
    setShowAddModal(true);
  };

  const closeForm = () => {
    setShowAddModal(false);
    setIsEditing(false);
    setSelectedDonation(null);
    setDonDate("");
    setDonCharityId("");
    setDonCategory("money");
    setDonAmount("");
    setDonNotes("");
    setValQuery("");
    setValSuggestions([]);
    setItemName("");
    setReceiptKey("");
    setReceiptName("");
    setReceiptType("");
    setReceiptSize(0);
    setOcrSuggestion(null);
    setError("");
  };

  const charityMap = new Map(charities.map((c) => [c.id, c.name]));

  // Filters logic
  const filteredDonations = donations.filter((d: any) => {
    const dYear = d.year || (d.date ? new Date(d.date).getFullYear() : new Date().getFullYear());
    if (filterYear !== "all" && dYear.toString() !== filterYear) return false;
    if (filterCategory !== "all" && d.category !== filterCategory) return false;

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      const cName = (charityMap.get(d.charity_id) || "").toLowerCase();
      const notes = (d.notes || "").toLowerCase();
      return cName.includes(query) || notes.includes(query);
    }

    return true;
  });

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white font-display">
            Donation Records
          </h1>
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            Log and review your charitable contributions, physical item valuations, and mileage logs.
          </p>
        </div>
        <button
          onClick={() => setShowAddModal(true)}
          className="dt-btn-primary self-start sm:self-center flex items-center gap-1"
        >
          <Plus size={16} /> Add Donation
        </button>
      </div>

      {/* Filter and Search Bar */}
      <section className="bg-white dark:bg-slate-900 p-4 rounded-xl border border-slate-200 dark:border-slate-800 shadow-sm flex flex-col md:flex-row gap-4 items-center">
        <div className="relative flex-1 w-full">
          <Search className="absolute left-3 top-2.5 h-4.5 w-4.5 text-slate-400" />
          <input
            type="text"
            placeholder="Search by charity or notes..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="dt-input mt-0 pl-10"
          />
        </div>
        <div className="flex flex-wrap gap-2 w-full md:w-auto">
          <select
            value={filterYear}
            onChange={(e) => setFilterYear(e.target.value)}
            className="dt-input mt-0 py-1.5 text-sm flex-1 md:flex-initial"
          >
            <option value="all">All Years</option>
            {years.map((y) => (
              <option key={y} value={y.toString()}>
                {y}
              </option>
            ))}
          </select>
          <select
            value={filterCategory}
            onChange={(e) => setFilterCategory(e.target.value)}
            className="dt-input mt-0 py-1.5 text-sm flex-1 md:flex-initial"
          >
            <option value="all">All Categories</option>
            <option value="money">Money</option>
            <option value="items">Items</option>
            <option value="mileage">Mileage</option>
          </select>
        </div>
      </section>

      {/* Table List */}
      <section className="bg-white dark:bg-slate-900 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm overflow-hidden">
        {filteredDonations.length === 0 ? (
          <div className="text-center py-12 text-slate-500">
            No donations match the current search or filters.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600 dark:text-slate-300">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-900/50 text-slate-500 uppercase text-xs tracking-wider">
                  <th className="py-3 px-4 font-medium">Date</th>
                  <th className="py-3 px-4 font-medium">Charity</th>
                  <th className="py-3 px-4 font-medium">Category</th>
                  <th className="py-3 px-4 font-medium text-right">Value</th>
                  <th className="py-3 px-4 font-medium text-center">Receipt</th>
                  <th className="py-3 px-4 font-medium text-center">Status</th>
                  <th className="py-3 px-4 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 dark:divide-slate-800/60">
                {filteredDonations.map((d: any) => {
                  const matchedReceipts = receipts.filter((r) => r.donation_id === d.id);
                  const syncPending = d.sync_status === "pending";

                  return (
                    <tr key={d.id} className="hover:bg-slate-50/40 dark:hover:bg-slate-800/10">
                      <td className="py-3.5 px-4 font-medium text-slate-900 dark:text-white">
                        {d.date}
                      </td>
                      <td className="py-3.5 px-4 font-medium truncate max-w-[200px]">
                        {charityMap.get(d.charity_id) || "Unknown Charity"}
                      </td>
                      <td className="py-3.5 px-4 capitalize">
                        {d.category || "money"}
                      </td>
                      <td className="py-3.5 px-4 text-right font-bold text-slate-900 dark:text-white">
                        {d.amount !== null ? formatCurrency(d.amount) : "$0.00"}
                      </td>
                      <td className="py-3.5 px-4 text-center">
                        {matchedReceipts.length > 0 ? (
                          <span className="inline-flex items-center justify-center h-6 px-2 rounded-md bg-indigo-50 dark:bg-indigo-950/30 text-xs font-semibold text-indigo-600 dark:text-indigo-400 border border-indigo-100/20">
                            {matchedReceipts.length} receipt{matchedReceipts.length > 1 && "s"}
                          </span>
                        ) : d.category === "items" ? (
                          <span className="text-slate-400 text-xs">-</span>
                        ) : (
                          <span className="inline-flex items-center justify-center h-6 px-2 rounded-md bg-rose-50 dark:bg-rose-950/20 text-xs font-semibold text-rose-600 dark:text-rose-450 border border-rose-100/20">
                            Missing
                          </span>
                        )}
                      </td>
                      <td className="py-3.5 px-4 text-center">
                        {syncPending ? (
                          <span className="inline-flex items-center justify-center h-5 px-2 rounded-md bg-yellow-50 dark:bg-yellow-950/30 text-xs font-semibold text-yellow-600 dark:text-yellow-450">
                            Pending
                          </span>
                        ) : (
                          <span className="inline-flex items-center justify-center h-5 px-2 rounded-md bg-green-50 dark:bg-green-950/30 text-xs font-semibold text-green-600 dark:text-green-400">
                            Synced
                          </span>
                        )}
                      </td>
                      <td className="py-3.5 px-4 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() => setSelectedDonation(d)}
                            className="p-1.5 rounded-lg border border-slate-200 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                          >
                            <Eye size={15} />
                          </button>
                          <button
                            onClick={() => openEdit(d)}
                            className="p-1.5 rounded-lg border border-slate-200 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                          >
                            <Edit3 size={15} />
                          </button>
                          <button
                            onClick={() => handleDeleteDonation(d.id)}
                            className="p-1.5 rounded-lg border border-rose-250 bg-rose-50 hover:bg-rose-100 dark:border-rose-900/50 dark:bg-rose-950/20 text-rose-600 dark:text-rose-400"
                          >
                            <Trash2 size={15} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* Add / Edit Donation Modal */}
      {showAddModal && (
        <div className="dt-modal-wrap">
          <div className="dt-modal max-w-2xl mt-8">
            <div className="flex items-center justify-between border-b border-slate-150 pb-3 dark:border-slate-800 mb-4">
              <h3 className="text-lg font-bold text-slate-900 dark:text-white font-display">
                {isEditing ? "Edit Donation Record" : "Log New Donation"}
              </h3>
              <button onClick={closeForm} className="text-slate-500 hover:text-slate-700">
                <X size={20} />
              </button>
            </div>

            {error && (
              <div className="rounded-md bg-rose-50 dark:bg-rose-950/20 p-3 mb-4 border border-rose-200 dark:border-rose-900/50">
                <p className="text-sm text-rose-600 dark:text-rose-400 font-medium">{error}</p>
              </div>
            )}

            {/* OCR Suggestion Bar */}
            {ocrSuggestion && (
              <div className="mb-4 p-4 rounded-xl border border-indigo-200 bg-indigo-50/50 dark:border-indigo-900 dark:bg-indigo-950/30 flex items-start justify-between gap-4">
                <div className="space-y-1">
                  <span className="inline-flex items-center gap-1 text-xs font-bold text-indigo-600 dark:text-indigo-400 uppercase tracking-wider">
                    <Sparkles size={14} /> Mistral OCR Suggestion
                  </span>
                  <p className="text-sm text-slate-700 dark:text-slate-350">
                    Extracted details:{" "}
                    <strong>
                      {ocrSuggestion.organizationName || "Unknown Org"} •{" "}
                      {ocrSuggestion.amountUsd ? `$${ocrSuggestion.amountUsd}` : "Non-cash"}
                    </strong>
                  </p>
                </div>
                <button
                  type="button"
                  onClick={applyOcrSuggestion}
                  className="dt-btn-primary py-1 px-3 text-xs flex items-center gap-1 shrink-0 bg-indigo-600 hover:bg-indigo-700 text-white"
                >
                  Apply Suggestion
                </button>
              </div>
            )}

            <form onSubmit={handleDonationSubmit} className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className="dt-label">Date of Donation</label>
                  <input
                    type="date"
                    required
                    value={donDate}
                    onChange={(e) => setDonDate(e.target.value)}
                    className="dt-input"
                  />
                </div>
                <div>
                  <label className="dt-label">Beneficiary Charity</label>
                  <select
                    required
                    value={donCharityId}
                    onChange={(e) => setDonCharityId(e.target.value)}
                    className="dt-input"
                  >
                    <option value="">Select a Charity</option>
                    {charities.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="dt-label">Donation Category</label>
                  <select
                    value={donCategory}
                    onChange={(e) => setDonCategory(e.target.value)}
                    className="dt-input"
                  >
                    <option value="money">Money</option>
                    <option value="items">Physical Goods (Items)</option>
                    <option value="mileage">Charity Mileage</option>
                  </select>
                </div>
                <div>
                  <label className="dt-label">
                    {donCategory === "mileage" ? "Total Miles Driven" : "Estimated Value (USD)"}
                  </label>
                  <input
                    type="number"
                    step="any"
                    value={donAmount}
                    onChange={(e) => setDonAmount(e.target.value)}
                    placeholder={donCategory === "mileage" ? "e.g. 45" : "e.g. 150"}
                    className="dt-input"
                  />
                </div>

                {/* Physical Goods Valuation Suggestion Input */}
                {donCategory === "items" && (
                  <div className="sm:col-span-2 p-4 rounded-xl border border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-900/50 space-y-3">
                    <label className="dt-label mt-0">Valuation Guide Search</label>
                    <div className="relative">
                      <input
                        type="text"
                        placeholder="Search valuation catalog (e.g. 'dresser', 'coat')..."
                        value={valQuery}
                        onChange={(e) => handleValuationSuggest(e.target.value)}
                        className="dt-input mt-0"
                      />
                      {valSuggestions.length > 0 && (
                        <div className="absolute left-0 right-0 top-full z-50 mt-1 max-h-48 overflow-y-auto border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-900 divide-y divide-slate-100 dark:divide-slate-800 text-sm shadow-lg">
                          {valSuggestions.map((s: any, idx) => (
                            <button
                              type="button"
                              key={idx}
                              onClick={() => handleSelectValuation(s)}
                              className="w-full text-left p-2.5 hover:bg-slate-50 dark:hover:bg-slate-800/40 text-slate-800 dark:text-slate-200 flex justify-between"
                            >
                              <span>{s[0]}</span>
                              <span className="font-semibold text-indigo-600 dark:text-indigo-400">
                                ${s[1]} - ${s[2]}
                              </span>
                            </button>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {/* Upload Receipt Section (only if not editing or when creating a new donation) */}
                {!isEditing && (
                  <div className="sm:col-span-2">
                    <label className="dt-label">Attach Donation Receipt</label>
                    <div className="mt-1 flex items-center justify-center rounded-lg border border-dashed border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 p-4 text-center">
                      <div className="space-y-1">
                        <Upload className="mx-auto h-8 w-8 text-slate-400" />
                        <div className="flex text-sm text-slate-600 dark:text-slate-400 justify-center">
                          <label className="relative cursor-pointer rounded-md font-semibold text-indigo-600 dark:text-indigo-400 hover:text-indigo-500 focus-within:outline-none">
                            <span>Upload receipt file</span>
                            <input
                              type="file"
                              accept="image/*,.pdf"
                              onChange={handleReceiptUpload}
                              className="sr-only"
                            />
                          </label>
                        </div>
                        <p className="text-xs text-slate-500">
                          {receiptName ? `Selected: ${receiptName}` : "Supports JPEG, PNG, WEBP, PDF up to 10MB"}
                        </p>
                      </div>
                    </div>
                    {uploadingReceipt && (
                      <p className="text-xs text-indigo-600 dark:text-indigo-400 mt-1 flex items-center gap-1 animate-pulse">
                        <Sparkles size={12} className="animate-spin" /> Uploading and running AI OCR analysis...
                      </p>
                    )}
                  </div>
                )}

                <div className="sm:col-span-2">
                  <label className="dt-label">Notes</label>
                  <textarea
                    rows={3}
                    value={donNotes}
                    onChange={(e) => setDonNotes(e.target.value)}
                    placeholder="e.g. 5 boxes of clothes, or standard direct bank draft memo"
                    className="dt-input"
                  />
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-4 border-t border-slate-100 dark:border-slate-800">
                <button type="button" onClick={closeForm} className="dt-btn-secondary">
                  Cancel
                </button>
                <button type="submit" className="dt-btn-primary">
                  {isEditing ? "Save Changes" : "Log Donation"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* View Donation Details Modal */}
      {selectedDonation && !isEditing && (
        <div className="dt-modal-wrap">
          <div className="dt-modal max-w-xl">
            <div className="flex items-center justify-between border-b border-slate-150 pb-3 dark:border-slate-800 mb-4">
              <h3 className="text-lg font-bold text-slate-900 dark:text-white font-display flex items-center gap-1.5">
                <FileText size={18} className="text-indigo-500" />
                Donation Details
              </h3>
              <button onClick={() => setSelectedDonation(null)} className="text-slate-500 hover:text-slate-700">
                <X size={20} />
              </button>
            </div>

            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase flex items-center gap-1">
                    <Calendar size={13} /> Date
                  </p>
                  <p className="text-sm font-bold text-slate-900 dark:text-white mt-0.5">
                    {selectedDonation.date}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase flex items-center gap-1">
                    <DollarSign size={13} /> Amount / Value
                  </p>
                  <p className="text-sm font-bold text-slate-900 dark:text-white mt-0.5">
                    {selectedDonation.amount !== null ? formatCurrency(selectedDonation.amount) : "$0.00"}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Category</p>
                  <p className="text-sm font-semibold text-slate-850 dark:text-slate-200 mt-0.5 capitalize">
                    {selectedDonation.category || "money"}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Charity</p>
                  <p className="text-sm font-semibold text-slate-850 dark:text-slate-200 mt-0.5">
                    {charityMap.get(selectedDonation.charity_id) || "Unknown Charity"}
                  </p>
                </div>
              </div>

              {selectedDonation.notes && (
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Notes</p>
                  <p className="text-sm text-slate-700 dark:text-slate-300 mt-0.5 whitespace-pre-line bg-slate-50 dark:bg-slate-900/50 p-3 rounded-lg border border-slate-100 dark:border-slate-800">
                    {selectedDonation.notes}
                  </p>
                </div>
              )}

              {/* Receipt Download Box */}
              <div>
                <p className="text-xs text-slate-500 font-semibold uppercase mb-2">Attached Receipts</p>
                {receipts.filter((r) => r.donation_id === selectedDonation.id).length === 0 ? (
                  <div className="flex items-start gap-2 bg-rose-50/50 dark:bg-rose-950/10 p-3 rounded-lg border border-rose-100/20 text-xs text-rose-600 dark:text-rose-400">
                    <Info size={15} className="shrink-0 mt-0.5" />
                    <div>
                      <span className="font-semibold">Missing IRS compliance document</span>
                      <p className="mt-0.5">
                        IRS rules require a physical receipt or bank record to claim deductions for monetary and non-cash gifts.
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {receipts
                      .filter((r) => r.donation_id === selectedDonation.id)
                      .map((receipt) => {
                        const loading = downloadingReceiptId === receipt.id;
                        return (
                          <div
                            key={receipt.id}
                            className="flex items-center justify-between p-3 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/40 text-sm"
                          >
                            <div className="flex items-center gap-2">
                              <FileText className="text-indigo-500" size={18} />
                              <div>
                                <p className="font-semibold text-slate-900 dark:text-white truncate max-w-[200px]">
                                  {receipt.file_name || `Receipt-${receipt.id.substring(0,8)}`}
                                </p>
                                <p className="text-xs text-slate-500">
                                  {receipt.size ? `${Math.round(receipt.size / 1024)} KB` : ""}
                                  {receipt.is_encrypted && " • Encrypted"}
                                </p>
                              </div>
                            </div>
                            <button
                              onClick={() => handleDownloadReceipt(receipt)}
                              disabled={loading}
                              className="dt-btn-secondary py-1 px-3 text-xs flex items-center gap-1 bg-white dark:bg-slate-800"
                            >
                              {loading ? "Downloading..." : "Download"} <ExternalLink size={12} />
                            </button>
                          </div>
                        );
                      })}
                  </div>
                )}
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-4 border-t border-slate-100 dark:border-slate-800 mt-6">
              <button
                onClick={() => {
                  const don = selectedDonation;
                  setSelectedDonation(null);
                  openEdit(don);
                }}
                className="dt-btn-secondary"
              >
                Edit
              </button>
              <button onClick={() => setSelectedDonation(null)} className="dt-btn-primary">
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
