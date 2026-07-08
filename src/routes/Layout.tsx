import React, { useState, useEffect } from "react";
import { Link, useNavigate, useLocation } from "react-router";
import { Sync } from "../services/sync";
import { getCurrentUser, clearCurrentUser } from "../services/currentUser";
import { apiJson } from "../services/apiClient";
import { Cloud, CloudOff, Menu, X, LogOut, Plus } from "lucide-react";

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  const [pendingChanges, setPendingChanges] = useState(0);
  const [isSyncing, setIsSyncing] = useState(false);
  const user = getCurrentUser();

  useEffect(() => {
    const handleOnlineStatus = () => {
      setIsOnline(navigator.onLine);
      if (navigator.onLine) {
        handleManualSync();
      }
    };
    const updatePendingCount = async () => {
      if (user?.id) {
        const count = await Sync.countPendingChanges(user.id);
        setPendingChanges(count);
      }
    };

    window.addEventListener("online", handleOnlineStatus);
    window.addEventListener("offline", handleOnlineStatus);
    window.addEventListener("sync-queue-changed", updatePendingCount);

    updatePendingCount();

    return () => {
      window.removeEventListener("online", handleOnlineStatus);
      window.removeEventListener("offline", handleOnlineStatus);
      window.removeEventListener("sync-queue-changed", updatePendingCount);
    };
  }, [user?.id]);

  const handleManualSync = async () => {
    setIsSyncing(true);
    try {
      await Sync.pushChanges();
      await Sync.pullChanges();
      if (user?.id) {
        const count = await Sync.countPendingChanges(user.id);
        setPendingChanges(count);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleLogout = async () => {
    try {
      await apiJson("/auth/logout", { method: "POST" });
    } catch (e) {
      // ignore
    }
    clearCurrentUser();
    navigate("/login");
  };

  const navLinks = [
    { name: "Home", path: "/" },
    { name: "Donations", path: "/donations" },
    { name: "Charities", path: "/charities" },
    { name: "Reports", path: "/reports" },
    { name: "Personal", path: "/personal" },
  ];

  return (
    <div className="flex min-h-screen flex-col bg-slate-100 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
      <header className="sticky top-0 z-40 border-b border-slate-200/80 bg-white/90 dark:border-slate-800/80 dark:bg-slate-900/90 backdrop-blur">
        <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
          <div className="flex h-16 items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-2 sm:gap-3">
              <button
                onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                className="lg:hidden inline-flex h-9 w-9 items-center justify-center rounded-xl border border-slate-200 text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800"
              >
                {mobileMenuOpen ? <X size={20} /> : <Menu size={20} />}
              </button>
              
              <div className="inline-flex h-9 w-9 items-center justify-center rounded-xl bg-indigo-600 text-white dark:bg-indigo-500">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  className="h-5 w-5"
                >
                  <rect x="4" y="2" width="16" height="20" rx="2"></rect>
                  <line x1="8" y1="6" x2="16" y2="6"></line>
                  <line x1="16" y1="14" x2="16" y2="18"></line>
                  <path d="M16 10h.01"></path>
                  <path d="M12 10h.01"></path>
                  <path d="M8 10h.01"></path>
                  <path d="M12 14h.01"></path>
                  <path d="M8 14h.01"></path>
                  <path d="M12 18h.01"></path>
                  <path d="M8 18h.01"></path>
                </svg>
              </div>
              <span className="truncate text-base font-semibold tracking-tight text-slate-900 dark:text-slate-100 sm:text-lg">
                Deductible Tracker
              </span>

              <nav className="ml-3 hidden items-center gap-1 lg:flex lg:ml-5">
                {navLinks.map((link) => (
                  <Link
                    key={link.path}
                    to={link.path}
                    className={`inline-flex items-center rounded-lg px-3 py-2 text-sm font-medium transition ${
                      location.pathname === link.path
                        ? "bg-indigo-50 text-indigo-700 dark:bg-slate-800 dark:text-indigo-400"
                        : "text-slate-700 hover:bg-indigo-50 hover:text-indigo-700 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-indigo-400"
                    }`}
                  >
                    {link.name}
                  </Link>
                ))}
              </nav>
            </div>

            <div className="flex items-center gap-2 sm:gap-3">
              {/* Sync Status Badge */}
              <div className="flex items-center rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-xs text-slate-500 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400">
                {isOnline ? (
                  <>
                    <Cloud className={`mr-1 h-4 w-4 text-green-500 ${isSyncing ? 'animate-spin' : ''}`} />
                    <span>Online</span>
                    {pendingChanges > 0 && (
                      <span className="ml-1 font-semibold">• {pendingChanges} pending</span>
                    )}
                    {pendingChanges > 0 && (
                      <button
                        onClick={handleManualSync}
                        disabled={isSyncing}
                        className="ml-2 font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
                      >
                        Sync now
                      </button>
                    )}
                  </>
                ) : (
                  <>
                    <CloudOff className="mr-1 h-4 w-4 text-red-500" />
                    <span>Offline</span>
                    {pendingChanges > 0 && (
                      <span className="ml-1 font-semibold">• {pendingChanges} pending</span>
                    )}
                  </>
                )}
              </div>

              {user && (
                <div className="hidden lg:flex items-center gap-2">
                  <button
                    onClick={() => navigate("/donations?new=true")}
                    className="inline-flex items-center rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                  >
                    <Plus size={16} className="mr-1" /> Add Donation
                  </button>
                  <button
                    onClick={handleLogout}
                    className="inline-flex items-center rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
                  >
                    <LogOut size={16} className="mr-1" /> Logout
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Mobile Navigation Menu */}
        {mobileMenuOpen && (
          <div className="absolute left-0 right-0 top-full z-50 border-t border-slate-200 bg-white shadow-xl dark:border-slate-800 dark:bg-slate-900 lg:hidden h-[calc(100vh-64px)] overflow-y-auto">
            <div className="space-y-1 px-3 py-3">
              <div className="pb-2">
                <button
                  onClick={() => {
                    setMobileMenuOpen(false);
                    navigate("/donations?new=true");
                  }}
                  className="w-full inline-flex items-center justify-center rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600"
                >
                  <Plus size={16} className="mr-1" /> Add Donation
                </button>
              </div>

              {navLinks.map((link) => (
                <Link
                  key={link.path}
                  to={link.path}
                  onClick={() => setMobileMenuOpen(false)}
                  className={`block rounded-lg px-3 py-2 text-sm font-medium ${
                    location.pathname === link.path
                      ? "bg-indigo-50 text-indigo-700 dark:bg-slate-800 dark:text-indigo-400"
                      : "text-slate-700 hover:bg-indigo-50 hover:text-indigo-700 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-indigo-400"
                  }`}
                >
                  {link.name}
                </Link>
              ))}

              <button
                onClick={() => {
                  setMobileMenuOpen(false);
                  handleLogout();
                }}
                className="mt-2 block w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-left text-sm font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
              >
                Logout
              </button>
            </div>
          </div>
        )}
      </header>

      <main className="flex-1 overflow-auto px-4 py-6 sm:px-6 sm:py-8 lg:px-8">
        <div className="mx-auto max-w-7xl">
          {children}
        </div>
      </main>
    </div>
  );
}
