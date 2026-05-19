import React, { Suspense } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";

const Setup = React.lazy(() => import("./routes/Setup"));
const Dashboard = React.lazy(() => import("./routes/Dashboard"));
const ProjectDetail = React.lazy(() => import("./routes/ProjectDetail"));
const RunLive = React.lazy(() => import("./routes/RunLive"));
const RunHistorical = React.lazy(() => import("./routes/RunHistorical"));
const Settings = React.lazy(() => import("./routes/Settings"));
// S-06 Launch Modal: not a route — rendered as overlay within route components

export default function App() {
  return (
    <HashRouter>
      <Suspense fallback={null}>
        <Routes>
          <Route path="/setup" element={<Setup />} />
          <Route path="/projects" element={<Dashboard />} />
          <Route path="/projects/:projectId" element={<ProjectDetail />} />
          <Route path="/projects/:projectId/runs/:runId/live" element={<RunLive />} />
          <Route path="/projects/:projectId/runs/:runId" element={<RunHistorical />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/" element={<Navigate to="/projects" replace />} />
        </Routes>
      </Suspense>
    </HashRouter>
  );
}
