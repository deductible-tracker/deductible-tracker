import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route, Navigate } from "react-router";
import { getCurrentUserId } from "./services/currentUser";
import Layout from "./routes/Layout";
import Login from "./routes/Login";
import Dashboard from "./routes/Dashboard";
import Donations from "./routes/Donations";
import Charities from "./routes/Charities";
import Reports from "./routes/Reports";
import Personal from "./routes/Personal";
import "./index.css";

// Helper component to protect auth routes and wrap in common layout
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const userId = getCurrentUserId();
  if (!userId) {
    return <Navigate to="/login" replace />;
  }
  return <Layout>{children}</Layout>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        {/* Public Routes */}
        <Route path="/login" element={<Login />} />

        {/* Protected Authenticated Routes */}
        <Route
          path="/"
          element={
            <ProtectedRoute>
              <Dashboard />
            </ProtectedRoute>
          }
        />
        <Route
          path="/donations"
          element={
            <ProtectedRoute>
              <Donations />
            </ProtectedRoute>
          }
        />
        <Route
          path="/charities"
          element={
            <ProtectedRoute>
              <Charities />
            </ProtectedRoute>
          }
        />
        <Route
          path="/reports"
          element={
            <ProtectedRoute>
              <Reports />
            </ProtectedRoute>
          }
        />
        <Route
          path="/personal"
          element={
            <ProtectedRoute>
              <Personal />
            </ProtectedRoute>
          }
        />

        {/* Catch-all */}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  </React.StrictMode>
);
