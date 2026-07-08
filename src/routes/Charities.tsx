import React, { useState, useEffect } from "react";
import db from "../services/db";
import { getCurrentUserId } from "../services/currentUser";
import { Sync } from "../services/sync";
import { apiJson, deleteCharityOnServer } from "../services/apiClient";
import { Search, Plus, Trash2, Edit3, Eye, X, Building, CheckCircle, Info } from "lucide-react";

export default function Charities() {
  const [charities, setCharities] = useState<any[]>([]);
  const [donations, setDonations] = useState<any[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<any[]>([]);
  const [searching, setSearching] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [selectedCharity, setSelectedCharity] = useState<any>(null);
  const [isEditing, setIsEditing] = useState(false);

  // Form states
  const [charityName, setCharityName] = useState("");
  const [charityEin, setCharityEin] = useState("");
  const [charityStreet, setCharityStreet] = useState("");
  const [charityCity, setCharityCity] = useState("");
  const [charityState, setCharityState] = useState("");
  const [charityZip, setCharityZip] = useState("");
  const [charityCategory, setCharityCategory] = useState("");
  const [error, setError] = useState("");

  const userId = getCurrentUserId();

  const loadData = async () => {
    if (!userId) return;
    const localChars = await db.charities.where("user_id").equals(userId).toArray();
    const localDons = await db.donations.where("user_id").equals(userId).toArray();
    setCharities(localChars);
    setDonations(localDons.filter((d: any) => !d.deleted));
  };

  useEffect(() => {
    loadData();
    window.addEventListener("sync-queue-changed", loadData);
    return () => {
      window.removeEventListener("sync-queue-changed", loadData);
    };
  }, [userId]);

  const handleProPublicaSearch = async () => {
    if (!searchQuery.trim()) return;
    setSearching(true);
    setSearchResults([]);
    try {
      const { res, data } = await apiJson(`/api/charities/search?q=${encodeURIComponent(searchQuery)}`);
      if (res.ok) {
        setSearchResults(data);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setSearching(false);
    }
  };

  const handleImportCharity = async (searchHit: any) => {
    setError("");
    try {
      // 1. Fetch full details by EIN
      const { res, data } = await apiJson(`/api/charities/lookup/${searchHit.ein}`);
      const details = res.ok && data ? data : searchHit;

      const newCharity = {
        id: crypto.randomUUID(),
        user_id: userId,
        name: details.name,
        ein: details.ein || null,
        category: details.category || null,
        status: details.status || null,
        classification: details.classification || null,
        nonprofit_type: details.nonprofit_type || null,
        deductibility: details.deductibility || null,
        street: details.street || null,
        city: details.city || null,
        state: details.state || null,
        zip: details.zip || null,
        cached_at: Date.now(),
      };

      // 2. Insert locally and queue sync action
      await Sync.queueAction("charities", newCharity, "create");
      await loadData();
      setSearchQuery("");
      setSearchResults([]);
      setShowAddModal(false);
    } catch (e: any) {
      setError(e.message || "Failed to import charity.");
    }
  };

  const handleCustomCharitySubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (!charityName.trim()) {
      setError("Charity name is required.");
      return;
    }

    const charityData = {
      id: isEditing ? selectedCharity.id : crypto.randomUUID(),
      user_id: userId,
      name: charityName.trim(),
      ein: charityEin.trim() || null,
      street: charityStreet.trim() || null,
      city: charityCity.trim() || null,
      state: charityState.trim() || null,
      zip: charityZip.trim() || null,
      category: charityCategory.trim() || null,
      status: isEditing ? selectedCharity.status : "active",
      classification: isEditing ? selectedCharity.classification : "Custom Organization",
      deductibility: isEditing ? selectedCharity.deductibility : "Deductible",
      cached_at: Date.now(),
    };

    try {
      await Sync.queueAction("charities", charityData, isEditing ? "update" : "create");
      await loadData();
      closeForm();
    } catch (e: any) {
      setError(e.message || "Failed to save charity.");
    }
  };

  const handleDeleteCharity = async (id: string) => {
    const hasDons = donations.some((d) => d.charity_id === id);
    if (hasDons) {
      alert("Cannot delete charity with active donations associated with it.");
      return;
    }

    if (!confirm("Are you sure you want to delete this charity?")) return;

    try {
      // Delete on server
      await deleteCharityOnServer(id);
      // Delete locally
      await db.charities.delete(id);
      await loadData();
      setSelectedCharity(null);
    } catch (e: any) {
      alert(e.message || "Failed to delete charity.");
    }
  };

  const openEdit = (charity: any) => {
    setSelectedCharity(charity);
    setIsEditing(true);
    setCharityName(charity.name);
    setCharityEin(charity.ein || "");
    setCharityStreet(charity.street || "");
    setCharityCity(charity.city || "");
    setCharityState(charity.state || "");
    setCharityZip(charity.zip || "");
    setCharityCategory(charity.category || "");
    setShowAddModal(true);
  };

  const closeForm = () => {
    setShowAddModal(false);
    setIsEditing(false);
    setCharityName("");
    setCharityEin("");
    setCharityStreet("");
    setCharityCity("");
    setCharityState("");
    setCharityZip("");
    setCharityCategory("");
    setError("");
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white font-display">
            Charities Directory
          </h1>
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            Lookup qualified 501(c)(3) charities or add custom organizations.
          </p>
        </div>
        <button
          onClick={() => setShowAddModal(true)}
          className="dt-btn-primary self-start sm:self-center flex items-center gap-1"
        >
          <Plus size={16} /> Add Charity
        </button>
      </div>

      {/* Directory Grid */}
      <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {charities.length === 0 ? (
          <div className="col-span-full text-center py-12 bg-white dark:bg-slate-900 rounded-2xl border border-slate-200 dark:border-slate-800 text-slate-500">
            No charities registered yet. Click "Add Charity" to register one.
          </div>
        ) : (
          charities.map((char: any) => {
            const donCount = donations.filter((d) => d.charity_id === char.id).length;
            return (
              <article
                key={char.id}
                className="bg-white dark:bg-slate-900 p-5 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm flex flex-col justify-between"
              >
                <div>
                  <div className="flex items-start justify-between gap-2">
                    <h3 className="font-bold text-slate-900 dark:text-white text-base leading-snug line-clamp-2">
                      {char.name}
                    </h3>
                    <span className="shrink-0 text-xs px-2 py-0.5 rounded-full bg-indigo-50 text-indigo-600 dark:bg-indigo-950/50 dark:text-indigo-400 font-medium capitalize">
                      {char.status || "active"}
                    </span>
                  </div>
                  {char.ein && (
                    <p className="text-xs text-slate-500 mt-1">EIN: {char.ein}</p>
                  )}
                  {char.city && (
                    <p className="text-xs text-slate-600 dark:text-slate-400 mt-2">
                      {char.city}, {char.state || ""}
                    </p>
                  )}
                </div>

                <div className="mt-5 pt-3 border-t border-slate-100 dark:border-slate-800 flex items-center justify-between">
                  <span className="text-xs font-semibold text-slate-500">
                    {donCount} donation{donCount !== 1 && "s"}
                  </span>
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={() => setSelectedCharity(char)}
                      className="p-1.5 rounded-lg border border-slate-200 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                    >
                      <Eye size={15} />
                    </button>
                    <button
                      onClick={() => openEdit(char)}
                      className="p-1.5 rounded-lg border border-slate-200 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                    >
                      <Edit3 size={15} />
                    </button>
                    <button
                      onClick={() => handleDeleteCharity(char.id)}
                      className="p-1.5 rounded-lg border border-rose-200 bg-rose-50 hover:bg-rose-100 dark:border-rose-900/50 dark:bg-rose-950/20 text-rose-600 dark:text-rose-400"
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
              </article>
            );
          })
        )}
      </div>

      {/* Add / Edit Charity Modal */}
      {showAddModal && (
        <div className="dt-modal-wrap">
          <div className="dt-modal max-w-2xl mt-8">
            <div className="flex items-center justify-between border-b border-slate-150 pb-3 dark:border-slate-800 mb-4">
              <h3 className="text-lg font-bold text-slate-900 dark:text-white font-display">
                {isEditing ? "Edit Charity" : "Add New Charity"}
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

            {/* ProPublica Online Search (only if adding new) */}
            {!isEditing && (
              <div className="mb-6 p-4 rounded-xl border border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-900/50 space-y-3">
                <h4 className="text-sm font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1">
                  <Search size={16} /> Import from IRS Registry
                </h4>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="Search by charity name or EIN..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="dt-input mt-0"
                    onKeyDown={(e) => e.key === "Enter" && handleProPublicaSearch()}
                  />
                  <button
                    onClick={handleProPublicaSearch}
                    disabled={searching}
                    className="dt-btn-secondary"
                  >
                    {searching ? "Searching..." : "Search"}
                  </button>
                </div>

                {searchResults.length > 0 && (
                  <div className="max-h-60 overflow-y-auto border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-900 divide-y divide-slate-100 dark:divide-slate-800">
                    {searchResults.map((hit: any) => (
                      <div
                        key={hit.ein}
                        className="p-3 flex items-center justify-between gap-4 hover:bg-slate-50 dark:hover:bg-slate-800/40 text-sm"
                      >
                        <div>
                          <p className="font-semibold text-slate-950 dark:text-white">{hit.name}</p>
                          <p className="text-xs text-slate-500">
                            EIN: {hit.ein} {hit.city && `| ${hit.city}, ${hit.state || ""}`}
                          </p>
                        </div>
                        <button
                          onClick={() => handleImportCharity(hit)}
                          className="dt-btn-primary py-1 px-3 text-xs flex items-center gap-1 shrink-0"
                        >
                          Import
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Manual Custom Form */}
            <form onSubmit={handleCustomCharitySubmit} className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className="dt-label">Organization Name</label>
                  <input
                    type="text"
                    required
                    value={charityName}
                    onChange={(e) => setCharityName(e.target.value)}
                    placeholder="e.g. Local Food Bank"
                    className="dt-input"
                  />
                </div>
                <div>
                  <label className="dt-label">EIN (Tax ID)</label>
                  <input
                    type="text"
                    value={charityEin}
                    onChange={(e) => setCharityEin(e.target.value)}
                    placeholder="e.g. 12-3456789"
                    className="dt-input"
                  />
                </div>
                <div className="sm:col-span-2">
                  <label className="dt-label">Street Address</label>
                  <input
                    type="text"
                    value={charityStreet}
                    onChange={(e) => setCharityStreet(e.target.value)}
                    placeholder="e.g. 123 Main St"
                    className="dt-input"
                  />
                </div>
                <div>
                  <label className="dt-label">City</label>
                  <input
                    type="text"
                    value={charityCity}
                    onChange={(e) => setCharityCity(e.target.value)}
                    placeholder="e.g. Austin"
                    className="dt-input"
                  />
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="dt-label">State</label>
                    <input
                      type="text"
                      value={charityState}
                      onChange={(e) => setCharityState(e.target.value)}
                      placeholder="TX"
                      maxLength={2}
                      className="dt-input"
                    />
                  </div>
                  <div>
                    <label className="dt-label">Zip Code</label>
                    <input
                      type="text"
                      value={charityZip}
                      onChange={(e) => setCharityZip(e.target.value)}
                      placeholder="78701"
                      className="dt-input"
                    />
                  </div>
                </div>
                <div>
                  <label className="dt-label">NTEE Category</label>
                  <input
                    type="text"
                    value={charityCategory}
                    onChange={(e) => setCharityCategory(e.target.value)}
                    placeholder="e.g. Human Services"
                    className="dt-input"
                  />
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-4 border-t border-slate-100 dark:border-slate-800">
                <button type="button" onClick={closeForm} className="dt-btn-secondary">
                  Cancel
                </button>
                <button type="submit" disabled={loading} className="dt-btn-primary">
                  {isEditing ? "Save Changes" : "Save Charity"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* View Charity Details Modal */}
      {selectedCharity && !isEditing && (
        <div className="dt-modal-wrap">
          <div className="dt-modal max-w-xl">
            <div className="flex items-center justify-between border-b border-slate-150 pb-3 dark:border-slate-800 mb-4">
              <h3 className="text-lg font-bold text-slate-900 dark:text-white font-display flex items-center gap-1.5">
                <Building size={18} className="text-indigo-500" />
                Charity Details
              </h3>
              <button onClick={() => setSelectedCharity(null)} className="text-slate-500 hover:text-slate-700">
                <X size={20} />
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <p className="text-xs text-slate-500 font-semibold uppercase">Name</p>
                <p className="text-base font-bold text-slate-900 dark:text-white mt-0.5">
                  {selectedCharity.name}
                </p>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">EIN</p>
                  <p className="text-sm font-medium text-slate-800 dark:text-slate-200 mt-0.5">
                    {selectedCharity.ein || "Not recorded"}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Status</p>
                  <p className="text-sm font-medium text-slate-800 dark:text-slate-200 mt-0.5 capitalize">
                    {selectedCharity.status || "active"}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Category</p>
                  <p className="text-sm font-medium text-slate-800 dark:text-slate-200 mt-0.5">
                    {selectedCharity.category || "General Charity"}
                  </p>
                </div>
                <div>
                  <p className="text-xs text-slate-500 font-semibold uppercase">Classification</p>
                  <p className="text-sm font-medium text-slate-800 dark:text-slate-200 mt-0.5">
                    {selectedCharity.classification || "Public Charity"}
                  </p>
                </div>
              </div>
              <div>
                <p className="text-xs text-slate-500 font-semibold uppercase">Address</p>
                <p className="text-sm font-medium text-slate-800 dark:text-slate-200 mt-0.5">
                  {selectedCharity.street
                    ? `${selectedCharity.street}, ${selectedCharity.city || ""}, ${
                        selectedCharity.state || ""
                      } ${selectedCharity.zip || ""}`
                    : selectedCharity.city
                    ? `${selectedCharity.city}, ${selectedCharity.state || ""}`
                    : "No address recorded"}
                </p>
              </div>

              <div className="pt-4 border-t border-slate-100 dark:border-slate-800">
                <p className="text-xs text-slate-500 font-semibold uppercase mb-2">IRS Deductibility status</p>
                <div className="flex items-start gap-2 bg-indigo-50/50 dark:bg-slate-850 p-3 rounded-lg border border-indigo-100/20 text-xs text-slate-600 dark:text-slate-300">
                  <CheckCircle size={15} className="text-indigo-600 shrink-0 mt-0.5" />
                  <div>
                    <span className="font-semibold text-slate-800 dark:text-slate-200">
                      {selectedCharity.deductibility || "Qualified Organization"}
                    </span>
                    <p className="mt-1">
                      Donations to this organization are generally tax-deductible up to IRS limit caps (60% AGI for cash, 30% for non-cash assets).
                    </p>
                  </div>
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-4 border-t border-slate-100 dark:border-slate-800 mt-6">
              <button
                onClick={() => {
                  const charity = selectedCharity;
                  setSelectedCharity(null);
                  openEdit(charity);
                }}
                className="dt-btn-secondary"
              >
                Edit
              </button>
              <button onClick={() => setSelectedCharity(null)} className="dt-btn-primary">
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
