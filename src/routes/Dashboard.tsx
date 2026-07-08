import React, { useState, useEffect } from "react";
import { Link } from "react-router";
import db from "../services/db";
import { getCurrentUserId, getCurrentUser } from "../services/currentUser";
import { calculateDonationFigures, formatCurrency } from "../services/donationFigures";
import { calculateTaxEstimates } from "../services/taxEstimates";
import { ArrowRight, Gift, CircleDollarSign, Navigation, Eye } from "lucide-react";

export default function Dashboard() {
  const [donations, setDonations] = useState<any[]>([]);
  const [charities, setCharities] = useState<any[]>([]);
  const [receipts, setReceipts] = useState<any[]>([]);
  const [figures, setFigures] = useState<any>(null);
  const [taxEstimates, setTaxEstimates] = useState<any>({ totalEstimated: 0 });
  const userId = getCurrentUserId();
  const userProfile = getCurrentUser();

  useEffect(() => {
    if (!userId) return;

    const loadData = async () => {
      const localDons = await db.donations.where("user_id").equals(userId).toArray();
      const activeDons = localDons.filter((d: any) => !d.deleted);
      const localChars = await db.charities.where("user_id").equals(userId).toArray();
      const localReceipts = await db.receipts.toArray();

      setDonations(activeDons);
      setCharities(localChars);
      setReceipts(localReceipts);

      const fig = calculateDonationFigures(activeDons);
      setFigures(fig);

      const estimates = await calculateTaxEstimates(
        activeDons,
        localChars,
        localReceipts,
        userProfile || {}
      );
      setTaxEstimates(estimates);
    };

    loadData();

    // Re-check periodically or listen for changes if needed
    window.addEventListener("sync-queue-changed", loadData);
    return () => {
      window.removeEventListener("sync-queue-changed", loadData);
    };
  }, [userId]);

  const charityMap = new Map(charities.map((c) => [c.id, c.name]));

  // Get most recent 5 donations
  const recentDonations = [...donations]
    .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())
    .slice(0, 5);

  return (
    <div className="space-y-8">
      {/* Workspace Overview Header Panel */}
      <section className="overflow-hidden rounded-2xl border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
        <div className="border-b border-slate-100 bg-gradient-to-r from-indigo-50/70 to-slate-50 px-5 py-5 sm:px-8 dark:border-slate-800 dark:from-indigo-950 dark:to-slate-900">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wider text-indigo-600 dark:text-indigo-400">
              Workspace Overview
            </p>
            <h1 className="mt-2 text-2xl font-bold tracking-tight text-slate-900 sm:text-3xl dark:text-slate-100 font-display">
              Track every donation
            </h1>
            <p className="mt-2 max-w-xl text-sm text-slate-600 dark:text-slate-300">
              Keep your giving organized year-round with searchable records, receipt workflows, and tax-ready reporting.
            </p>
          </div>
        </div>

        <div className="px-5 py-5 sm:px-8 sm:py-7">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            {/* Total Donations Summary */}
            <article className="rounded-xl border border-slate-200 bg-white p-6 dark:border-slate-800 dark:bg-slate-900/50 flex flex-col justify-between">
              <div>
                <p className="text-xs font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400">
                  Total Donations
                </p>
                <p className="mt-3 text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-slate-100 font-display">
                  {figures ? formatCurrency(figures.total.amount) : "$0.00"}
                </p>
              </div>
              <p className="mt-4 text-sm font-semibold text-emerald-600 dark:text-emerald-400">
                {formatCurrency(taxEstimates.totalEstimated)} in estimated tax savings
              </p>
            </article>

            {/* Category Breakdown */}
            <article className="rounded-xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
              <p className="text-xs font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400 mb-3">
                Breakdown by Type
              </p>
              <div className="grid gap-3 text-sm">
                <div className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-800">
                  <div className="flex items-center gap-2">
                    <Gift size={16} className="text-indigo-500" />
                    <span className="font-medium">Items</span>
                  </div>
                  <span className="font-semibold text-slate-900 dark:text-slate-100">
                    {figures ? formatCurrency(figures.items.amount) : "$0.00"}
                  </span>
                </div>
                <div className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-800">
                  <div className="flex items-center gap-2">
                    <CircleDollarSign size={16} className="text-indigo-500" />
                    <span className="font-medium">Money</span>
                  </div>
                  <span className="font-semibold text-slate-900 dark:text-slate-100">
                    {figures ? formatCurrency(figures.money.amount) : "$0.00"}
                  </span>
                </div>
                <div className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-800">
                  <div className="flex items-center gap-2">
                    <Navigation size={16} className="text-indigo-500" />
                    <span className="font-medium">Mileage</span>
                  </div>
                  <span className="font-semibold text-slate-900 dark:text-slate-100">
                    {figures ? formatCurrency(figures.mileage.amount) : "$0.00"}
                  </span>
                </div>
              </div>
            </article>
          </div>
        </div>
      </section>

      {/* Recent Donations List */}
      <section className="bg-white dark:bg-slate-900 p-6 rounded-2xl border border-slate-200 dark:border-slate-800 shadow-sm">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-slate-900 dark:text-white font-display">
            Recent Donations
          </h2>
          <Link
            to="/donations"
            className="inline-flex items-center gap-1 text-sm font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
          >
            View all <ArrowRight size={14} />
          </Link>
        </div>

        {recentDonations.length === 0 ? (
          <div className="text-center py-8 text-slate-500 dark:text-slate-400">
            No donations recorded yet. Get started by adding a donation.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600 dark:text-slate-300">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800 text-slate-500 uppercase text-xs tracking-wider">
                  <th className="py-3 px-2 font-medium">Date</th>
                  <th className="py-3 px-2 font-medium">Charity</th>
                  <th className="py-3 px-2 font-medium">Category</th>
                  <th className="py-3 px-2 font-medium text-right">Value</th>
                  <th className="py-3 px-2 text-right">Action</th>
                </tr>
              </thead>
              <tbody>
                {recentDonations.map((d: any) => (
                  <tr key={d.id} className="border-b border-slate-100 dark:border-slate-800/50 hover:bg-slate-50/50 dark:hover:bg-slate-800/20">
                    <td className="py-3 px-2 font-medium">{d.date || "N/A"}</td>
                    <td className="py-3 px-2 truncate max-w-[200px]">
                      {charityMap.get(d.charity_id) || "Unknown Charity"}
                    </td>
                    <td className="py-3 px-2 capitalize">{d.category || "money"}</td>
                    <td className="py-3 px-2 text-right font-semibold text-slate-900 dark:text-white">
                      {d.amount !== null ? formatCurrency(d.amount) : "N/A"}
                    </td>
                    <td className="py-3 px-2 text-right">
                      <Link
                        to={`/donations?view=${d.id}`}
                        className="inline-flex items-center justify-center p-1.5 rounded-lg border border-slate-200 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400"
                        title="View details"
                      >
                        <Eye size={16} />
                      </Link>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
