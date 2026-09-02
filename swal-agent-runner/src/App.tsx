import React, { useState, useEffect } from "react";
import {
  GestaltWasmBridge,
  RunSpec,
  RunReport
} from "./wasm/gestaltWasm";
import {
  Cpu,
  Activity,
  Database,
  Compass,
  Sliders,
  Menu,
  X,
  AlertTriangle,
  Play,
  Layers,
  Wifi,
  WifiOff
} from "lucide-react";

// Inlined features definition for UI representation
const INITIAL_FEATURES = [
  {
    id: "feat-ar-001",
    name: "Gestalt WASM Integration",
    priority: "P0",
    milestone: "Milestone 1",
    description: "Integration with @swal/gestalt-wasm to execute parallel runs and read event stream schemas.",
    dependencies: [],
    progress_pct: 10,
    status: "initial_draft",
    steps: ["Create type-safe typescript interfaces", "Implement GestaltWasmBridge", "Load WASM dynamically", "E2E testing"]
  },
  {
    id: "feat-ar-002",
    name: "WebContainer Sandboxed Environment",
    priority: "P1",
    milestone: "Milestone 1",
    description: "Bootstrapping WebContainer in browser to execute workspace builds and agent code.",
    dependencies: ["feat-ar-001"],
    progress_pct: 0,
    status: "pending",
    steps: ["Configure COOP/COEP isolation headers", "Mount workspace onto WebContainer", "Spawn terminal processes"]
  },
  {
    id: "feat-ar-003",
    name: "Isomorphic-Git Browser Client",
    priority: "P1",
    milestone: "Milestone 2",
    description: "Perform full git operations directly inside browser using memory/IndexedDB filesystems.",
    dependencies: ["feat-ar-002"],
    progress_pct: 0,
    status: "pending",
    steps: ["Initialize isomorphic-git with LightningFS", "Implement clone/checkout workflows", "Handle merge and conflict marking"]
  },
  {
    id: "feat-ar-004",
    name: "Event Bus & WebSocket Sync",
    priority: "P2",
    milestone: "Milestone 2",
    description: "Real-time synchronization and timeline streaming with Gestalt event bus (:8081) and WS server (:3001).",
    dependencies: ["feat-ar-001"],
    progress_pct: 0,
    status: "pending",
    steps: ["Connect to WebSocket", "Listen to timeline events", "Replay missing events via cursor API"]
  },
  {
    id: "feat-ar-005",
    name: "Offline-First PWA State & Cache",
    priority: "P2",
    milestone: "Milestone 3",
    description: "Implement Service Worker, app manifests, and IndexedDB state caching for resilient offline-first work.",
    dependencies: [],
    progress_pct: 0,
    status: "pending",
    steps: ["Register Service Worker with custom asset caching", "Design offline indicators", "Configure manifest.json shell"]
  },
  {
    id: "feat-ar-006",
    name: "Xavier Semantic Memory Integration",
    priority: "P3",
    milestone: "Milestone 3",
    description: "Direct communication with Xavier for semantic memory search (PRE-run context) and run archival (POST-run result).",
    dependencies: ["feat-ar-001"],
    progress_pct: 0,
    status: "pending",
    steps: ["Map Xavier client endpoints", "Formulate kind=execution payloads", "Feed context into agent runs"]
  }
];

export default function App() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [activeTab, setActiveTab] = useState("dashboard");
  const [features] = useState(INITIAL_FEATURES);
  const [wasmBridge] = useState(() => new GestaltWasmBridge());
  const [wasminited, setWasmInited] = useState(false);

  // In-memory stats
  const [stats, setStats] = useState({
    totalRuns: 3,
    successRate: 100,
    activeAgents: 4,
    isOnline: true
  });

  // Run orchestration forms
  const [taskText, setTaskText] = useState("Fix linting bugs in the checkout service");
  const [selectedAgent, setSelectedAgent] = useState("agy");
  const [parallelLimit, setParallelLimit] = useState(2);
  const [executing, setExecuting] = useState(false);
  const [runReport, setRunReport] = useState<RunReport | null>(null);

  // Event stream log
  const [logs, setLogs] = useState<string[]>([
    "System booted",
    "Tailwind v4 theme applied successfully",
    "Service worker registration skipped in dev mode"
  ]);

  useEffect(() => {
    // Initialize WASM Bridge
    wasmBridge.initialize().then((success) => {
      setWasmInited(success);
      if (success) {
        setLogs((prev) => [...prev, "Gestalt WASM Engine initialized successfully."]);
      }
    });

    // Simulated network status listener
    const handleOnline = () => setStats((prev) => ({ ...prev, isOnline: true }));
    const handleOffline = () => setStats((prev) => ({ ...prev, isOnline: false }));
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);
    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, [wasmBridge]);

  const handleExecuteRun = async (e: React.FormEvent) => {
    e.preventDefault();
    setExecuting(true);
    setLogs((prev) => [...prev, `Spawning execution run for: "${taskText}"`]);

    const spec: RunSpec = {
      base_ref: "main",
      task: taskText,
      agents: [
        { id: selectedAgent, command: selectedAgent, args: ["-p", taskText] },
        { id: "kimi", command: "kimi", args: ["-p", "optimize merge scope"] }
      ],
      max_parallel: parallelLimit,
      timeout: 120.0,
      push: true
    };

    setTimeout(async () => {
      try {
        const report = await wasmBridge.executeRunSpec(spec);
        setRunReport(report);
        setStats((prev) => ({
          ...prev,
          totalRuns: prev.totalRuns + 1,
          successRate: report.success ? Math.round(((prev.totalRuns + 1 - (report.conflicts.length > 0 ? 1 : 0)) / (prev.totalRuns + 1)) * 100) : prev.successRate
        }));
        setLogs((prev) => [
          ...prev,
          `Run ${report.run_id.substring(0, 8)} finished. Success: ${report.success}.`
        ]);
      } catch (err: any) {
        setLogs((prev) => [...prev, `Execution crash: ${err.message || err}`]);
      } finally {
        setExecuting(false);
      }
    }, 1500);
  };

  return (
    <div className="flex h-screen bg-slate-950 text-slate-100 overflow-hidden font-sans">
      {/* Mobile sidebar overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-40 bg-slate-950/80 backdrop-blur-sm lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Desktop & Mobile Sidebar */}
      <aside className={`
        fixed inset-y-0 left-0 z-50 flex flex-col w-64 bg-slate-900 border-r border-slate-800 transition-transform duration-300 transform
        lg:translate-x-0 lg:static lg:inset-auto
        ${sidebarOpen ? "translate-x-0" : "-translate-x-full"}
      `}>
        <div className="flex items-center justify-between h-16 px-6 border-b border-slate-800 bg-slate-900/50">
          <div className="flex items-center space-x-3">
            <Layers className="w-6 h-6 text-cyan-400" />
            <span className="text-lg font-bold tracking-wider text-slate-50 uppercase">SWAL Runner</span>
          </div>
          <button className="lg:hidden text-slate-400 hover:text-slate-100" onClick={() => setSidebarOpen(false)}>
            <X className="w-5 h-5" />
          </button>
        </div>

        <nav className="flex-1 p-4 space-y-1.5 overflow-y-auto">
          <button
            onClick={() => { setActiveTab("dashboard"); setSidebarOpen(false); }}
            className={`flex items-center w-full px-4 py-3 rounded-lg text-sm font-medium transition-colors ${activeTab === "dashboard" ? "bg-slate-800 text-cyan-400" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-100"}`}
          >
            <Activity className="w-5 h-5 mr-3" />
            Control Dashboard
          </button>
          <button
            onClick={() => { setActiveTab("roadmap"); setSidebarOpen(false); }}
            className={`flex items-center w-full px-4 py-3 rounded-lg text-sm font-medium transition-colors ${activeTab === "roadmap" ? "bg-slate-800 text-cyan-400" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-100"}`}
          >
            <Compass className="w-5 h-5 mr-3" />
            Feature Roadmap
          </button>
          <button
            onClick={() => { setActiveTab("orchestration"); setSidebarOpen(false); }}
            className={`flex items-center w-full px-4 py-3 rounded-lg text-sm font-medium transition-colors ${activeTab === "orchestration" ? "bg-slate-800 text-cyan-400" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-100"}`}
          >
            <Sliders className="w-5 h-5 mr-3" />
            Run Orchestration
          </button>
        </nav>

        <div className="p-4 border-t border-slate-800 bg-slate-900/30">
          <div className="flex items-center space-x-3 text-xs">
            <span className={`w-2.5 h-2.5 rounded-full ${wasminited ? "bg-emerald-500 animate-pulse" : "bg-red-500"}`} />
            <span className="text-slate-400">Gestalt WASM Engine</span>
          </div>
          <div className="flex items-center space-x-3 text-xs mt-2">
            <span className={`w-2.5 h-2.5 rounded-full ${stats.isOnline ? "bg-emerald-500" : "bg-amber-500"}`} />
            <span className="text-slate-400">{stats.isOnline ? "Ecosystem Bus Active" : "Offline Cache Mode"}</span>
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden bg-slate-950">
        <header className="flex items-center justify-between h-16 px-6 border-b border-slate-800 bg-slate-900/20">
          <button className="lg:hidden text-slate-400 hover:text-slate-100" onClick={() => setSidebarOpen(true)}>
            <Menu className="w-6 h-6" />
          </button>
          <div className="hidden lg:flex items-center space-x-3">
            <span className="text-xs px-2.5 py-1 rounded bg-slate-800 border border-slate-700 text-cyan-400 font-mono">React v19.0.0</span>
            <span className="text-xs px-2.5 py-1 rounded bg-slate-800 border border-slate-700 text-purple-400 font-mono">Tailwind v4.0</span>
          </div>
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2 text-xs bg-slate-900 px-3 py-1.5 rounded-full border border-slate-800">
              {stats.isOnline ? <Wifi className="w-4 h-4 text-emerald-400" /> : <WifiOff className="w-4 h-4 text-amber-400" />}
              <span className="text-slate-300">{stats.isOnline ? "Online" : "Offline"}</span>
            </div>
          </div>
        </header>

        <main className="flex-1 overflow-y-auto p-6 space-y-6">
          {activeTab === "dashboard" && (
            <div className="space-y-6">
              {/* Metric Row */}
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <div className="bg-slate-900 p-5 rounded-xl border border-slate-800 shadow-lg">
                  <div className="text-xs text-slate-400 font-medium uppercase tracking-wider">Total Actions Spanned</div>
                  <div className="text-3xl font-extrabold text-slate-100 mt-2 font-mono">{stats.totalRuns}</div>
                </div>
                <div className="bg-slate-900 p-5 rounded-xl border border-slate-800 shadow-lg">
                  <div className="text-xs text-slate-400 font-medium uppercase tracking-wider">Run Integration Success</div>
                  <div className="text-3xl font-extrabold text-emerald-400 mt-2 font-mono">{stats.successRate}%</div>
                </div>
                <div className="bg-slate-900 p-5 rounded-xl border border-slate-800 shadow-lg">
                  <div className="text-xs text-slate-400 font-medium uppercase tracking-wider">Active Swarm Agent Profiles</div>
                  <div className="text-3xl font-extrabold text-purple-400 mt-2 font-mono">{stats.activeAgents}</div>
                </div>
                <div className="bg-slate-900 p-5 rounded-xl border border-slate-800 shadow-lg">
                  <div className="text-xs text-slate-400 font-medium uppercase tracking-wider">Offline State Buffer</div>
                  <div className="text-3xl font-extrabold text-cyan-400 mt-2 font-mono">Clean</div>
                </div>
              </div>

              {/* Grid with App overview & quick logs */}
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                <div className="lg:col-span-2 bg-slate-900 p-6 rounded-xl border border-slate-800 space-y-4">
                  <h2 className="text-lg font-bold text-slate-50 flex items-center">
                    <Database className="w-5 h-5 mr-2 text-cyan-400" />
                    SWAL Universal PWA Runner
                  </h2>
                  <p className="text-sm text-slate-400 leading-relaxed">
                    This progressive web application is the core frontend platform for SWAL. Running browser-isolated subagents in WebContainers, synchronizing history via isomorphic-git, and evaluating actions through <strong>Gestalt WebAssembly Core</strong>. All operations are offline-first and stream telemetry directly to Xavier.
                  </p>
                  <div className="pt-4 border-t border-slate-800 grid grid-cols-2 gap-4 text-xs">
                    <div>
                      <span className="text-slate-500 block">Workspace Branch</span>
                      <span className="text-slate-300 font-mono">main-workspace-v4</span>
                    </div>
                    <div>
                      <span className="text-slate-500 block">Storage Layer</span>
                      <span className="text-slate-300 font-mono">IndexedDB + SQLite WS</span>
                    </div>
                  </div>
                </div>

                <div className="bg-slate-900 p-6 rounded-xl border border-slate-800 flex flex-col h-64 lg:h-auto">
                  <h3 className="text-sm font-bold text-slate-200 uppercase tracking-wider mb-3">Live Feed Log</h3>
                  <div className="flex-1 overflow-y-auto space-y-2 text-xs font-mono bg-slate-950 p-3 rounded-lg border border-slate-800">
                    {logs.map((log, i) => (
                      <div key={i} className="text-slate-400 border-l border-cyan-500/30 pl-2">
                        <span className="text-slate-600 mr-2">[{new Date().toLocaleTimeString()}]</span>
                        {log}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {activeTab === "roadmap" && (
            <div className="space-y-6">
              <div className="bg-slate-900 p-6 rounded-xl border border-slate-800">
                <h2 className="text-lg font-bold text-slate-100">Declared Features Implementation Status</h2>
                <p className="text-sm text-slate-400 mt-1">We prioritize Gestalt WASM integration first to guarantee reliable type bindings.</p>

                <div className="mt-6 space-y-6">
                  {features.map((feat) => (
                    <div key={feat.id} className="bg-slate-950 p-5 rounded-lg border border-slate-800 space-y-3">
                      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                        <div className="flex items-center space-x-3">
                          <span className="text-xs px-2 py-0.5 rounded bg-slate-800 text-slate-300 font-mono font-bold">{feat.id}</span>
                          <h3 className="text-sm font-bold text-slate-200">{feat.name}</h3>
                        </div>
                        <div className="flex items-center space-x-2">
                          <span className="text-xs px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">{feat.priority}</span>
                          <span className="text-xs px-2 py-0.5 rounded-full bg-slate-800 text-slate-400">{feat.milestone}</span>
                        </div>
                      </div>

                      <p className="text-xs text-slate-400 leading-relaxed">{feat.description}</p>

                      <div className="space-y-1">
                        <div className="flex justify-between text-[11px] text-slate-500">
                          <span>Overall Progress</span>
                          <span className="font-mono">{feat.progress_pct}%</span>
                        </div>
                        <div className="w-full bg-slate-800 h-2 rounded-full overflow-hidden">
                          <div className="bg-cyan-400 h-full transition-all duration-500" style={{ width: `${feat.progress_pct}%` }} />
                        </div>
                      </div>

                      <div className="text-[11px] space-y-1">
                        <div className="text-slate-500 font-medium">Core Steps Check:</div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-slate-400 font-mono">
                          {feat.steps.map((step, idx) => (
                            <div key={idx} className="flex items-center space-x-2">
                              <span className={`w-1.5 h-1.5 rounded-full ${idx < (feat.progress_pct / 25) ? "bg-cyan-400" : "bg-slate-700"}`} />
                              <span>{step}</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {activeTab === "orchestration" && (
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
              {/* Launcher Form */}
              <div className="lg:col-span-1 bg-slate-900 p-6 rounded-xl border border-slate-800 h-fit space-y-4">
                <h3 className="text-md font-bold text-slate-100 flex items-center">
                  <Play className="w-4 h-4 mr-2 text-cyan-400" />
                  Spawn WASM Agent Execution
                </h3>

                <form onSubmit={handleExecuteRun} className="space-y-4 text-xs">
                  <div>
                    <label className="block text-slate-400 mb-1">Target Task Statement</label>
                    <textarea
                      value={taskText}
                      onChange={(e) => setTaskText(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-lg p-2.5 text-slate-200 focus:outline-none focus:border-cyan-500"
                      rows={3}
                      required
                    />
                  </div>

                  <div>
                    <label className="block text-slate-400 mb-1">Primary Orchestrated Agent</label>
                    <select
                      value={selectedAgent}
                      onChange={(e) => setSelectedAgent(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-lg p-2.5 text-slate-200 focus:outline-none focus:border-cyan-500"
                    >
                      <option value="agy">agy (High Effort / Gemini 3.6)</option>
                      <option value="kimi">kimi (Generalist)</option>
                      <option value="codex">codex (Refactor Specialist)</option>
                      <option value="claude">claude (Architect)</option>
                    </select>
                  </div>

                  <div>
                    <label className="block text-slate-400 mb-1">Max Parallel Limit</label>
                    <input
                      type="number"
                      value={parallelLimit}
                      onChange={(e) => setParallelLimit(parseInt(e.target.value) || 1)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-lg p-2.5 text-slate-200 focus:outline-none focus:border-cyan-500"
                      min={1}
                      max={4}
                    />
                  </div>

                  <button
                    type="submit"
                    disabled={executing}
                    className={`w-full flex items-center justify-center py-2.5 rounded-lg font-bold text-slate-950 transition-colors ${executing ? "bg-slate-700 cursor-not-allowed" : "bg-cyan-400 hover:bg-cyan-300"}`}
                  >
                    {executing ? "Processing Workspace Merges..." : "Spawn Parallel Run"}
                  </button>
                </form>
              </div>

              {/* Execution Results View */}
              <div className="lg:col-span-2 bg-slate-900 p-6 rounded-xl border border-slate-800 min-h-[300px] flex flex-col">
                <h3 className="text-md font-bold text-slate-100 border-b border-slate-800 pb-3 flex items-center">
                  <Cpu className="w-5 h-5 mr-2 text-cyan-400" />
                  WASM Run Report Viewer
                </h3>

                <div className="flex-1 flex flex-col justify-center mt-4">
                  {executing ? (
                    <div className="text-center py-12 space-y-3">
                      <div className="w-8 h-8 border-4 border-cyan-400 border-t-transparent rounded-full animate-spin mx-auto" />
                      <p className="text-sm text-slate-400">Executing agent in isolated sandbox and evaluating mergeability...</p>
                    </div>
                  ) : runReport ? (
                    <div className="space-y-4 text-xs">
                      <div className="grid grid-cols-2 gap-4">
                        <div className="bg-slate-950 p-3 rounded border border-slate-800">
                          <span className="text-slate-500 block">Run Identifier</span>
                          <span className="text-slate-300 font-mono">{runReport.run_id}</span>
                        </div>
                        <div className="bg-slate-950 p-3 rounded border border-slate-800">
                          <span className="text-slate-500 block">Status / Duration</span>
                          <span className={`font-bold font-mono ${runReport.success ? "text-emerald-400" : "text-amber-400"}`}>
                            {runReport.success ? "SUCCESS" : "CONFLICTS DETECTED"} ({Math.round(runReport.duration_ms)}ms)
                          </span>
                        </div>
                      </div>

                      <div className="space-y-2">
                        <span className="text-slate-500 font-bold block">Dispatched Agents Outcomes</span>
                        {runReport.agents.map((ag) => (
                          <div key={ag.agent_id} className="bg-slate-950 p-3 rounded border border-slate-800">
                            <div className="flex justify-between font-mono font-bold text-slate-300">
                              <span>Profile: {ag.agent_id}</span>
                              <span className="text-cyan-400">{ag.duration_ms.toFixed(0)}ms</span>
                            </div>
                            <p className="text-slate-400 mt-1 font-mono">{ag.output}</p>
                            <div className="text-slate-500 text-[10px] mt-1">Changed files: {ag.changed_files.join(", ")}</div>
                          </div>
                        ))}
                      </div>

                      {runReport.conflicts.length > 0 && (
                        <div className="bg-amber-500/10 text-amber-400 p-3 rounded border border-amber-500/20 flex items-start space-x-3">
                          <AlertTriangle className="w-5 h-5 flex-shrink-0" />
                          <div>
                            <span className="font-bold">Real-time Merge Lock Conflict:</span>
                            <p className="mt-0.5 leading-relaxed">
                              Lock conflict detected on path <code className="font-mono bg-amber-500/20 px-1 rounded">{runReport.conflicts[0].path}</code>. Agent <code className="font-mono bg-amber-500/20 px-1 rounded">{runReport.conflicts[0].agent_id}</code> execution was gracefully rolled back using CleanSlateRetry.
                            </p>
                          </div>
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="text-center py-12 text-slate-500">
                      <Sliders className="w-12 h-12 mx-auto text-slate-700 mb-3" />
                      <p className="text-sm">No run reports loaded yet. Define a task and spawn an agent run to stream results.</p>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
