import React, { Suspense } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";

const Setup = React.lazy(() => import("./routes/Setup"));
const Dashboard = React.lazy(() => import("./routes/Dashboard"));
const ProjectDetail = React.lazy(() => import("./routes/ProjectDetail"));
const RunLive = React.lazy(() => import("./routes/RunLive"));
const RunHistorical = React.lazy(() => import("./routes/RunHistorical"));
const Settings = React.lazy(() => import("./routes/Settings"));

// S-06 LaunchModal is NOT a route — it is a modal overlay rendered inside
// ProjectDetail (S-03) and Dashboard (S-02). It never gets its own URL.

const suspenseFallback = <div style={{ background: "var(--bg-base)", minHeight: "100vh" }} />;

export default function App() {
  return (
    <HashRouter>
      <Suspense fallback={suspenseFallback}>
        <Routes>
          {/* S-01 Setup — shown when Claude CLI is missing */}
          <Route path="/setup" element={<Setup />} />

          {/* S-02 Dashboard — primary screen */}
          <Route path="/" element={<Dashboard />} />

          {/* S-03 Project Detail */}
          <Route path="/projects/:projectId" element={<ProjectDetail />} />

          {/* S-04 Run View (Live) */}
          <Route path="/runs/:runId/live" element={<RunLive />} />

          {/* S-05 Run View (Historical) */}
          <Route path="/runs/:runId/history" element={<RunHistorical />} />

          {/* S-07 Settings */}
          <Route path="/settings" element={<Settings />} />

          {/* Catch-all: redirect unknown routes to Dashboard */}
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </Suspense>
    </HashRouter>
  );
}
