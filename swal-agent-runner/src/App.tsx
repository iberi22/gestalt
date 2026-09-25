import React, { useState, useEffect } from "react";
import {
  GestaltWasmBridge,
  RunSpec,
  RunReport,
  AtlasTask,
  AtlasEvent,
  AtlasWsFrame
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
  WifiOff,
  RefreshCw,
  CheckCircle2,
  Clock,
  Radio
} from "lucide-react";

// Initial fallback tasks for Atlas ADE DAG when WS is offline or booting
const INITIAL_ATLAS_TASKS: AtlasTask[] = [
  {
    id: "task-001",
    session_id: "sess-atlas-root",
    title: "1. Scope: Long-Horizon Session Architecture",
    state: "READY",
    agent: "agy",
    attempts: 0,
    created_at: 1757500000,
    updated_at: 1757501000,
    deps: []
  },
  {
    id: "task-002",
    session_id: "sess-atlas-root",
    title: "2. Ground in CodeGraph Symbols (Gestalt VFS)",
    state: "BLOCKED",
    agent: "kimi",
    attempts: 0,
    created_at: 1757500100,
    updated_at: 1757501100,
    deps: ["task-001"]
  },
  {
    id: "task-003",
    session_id: "sess-atlas-root",
    title: "3. Execute Multi-Agent Verification Gate",
    state: "BLOCKED",
    agent: "claude",
    attempts: 0,
    created_at: 1757500200,
    updated_at: 1757501200,
    deps: ["task-002"]
  }
];

// Initial fallback events for Atlas ADE Console
const INITIAL_ATLAS_EVENTS: AtlasEvent[] = [
  {
    id: 101,
    kind: "task_dispatched",
    payload: "Task Ready Queue: Dispatched to Gestalt Worktrees",
    idempotency_key: "sodp-101",
    recorded_at: 1757501500
  },
  {
    id: 200,
    kind: "vfs_lock_acquired",
    payload: "File Island Locks: Active in Gestalt MemState",
    idempotency_key: "sodp-200",
    recorded_at: 1757501600
  },
  {
    id: 420,
    kind: "xavier_stream_sync",
    payload: "Xavier Memory Stream: Connected (:8006 kind=execution)",
    idempotency_key: "sodp-420",
    recorded_at: 1757501700
  }
];

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
    description: "Real-time synchronization and timeline streaming with Gestalt WS server (:3001) and Atlas ADE serve (:8080/ws).",
    dependencies: ["feat-ar-001"],
    progress_pct: 100,
    status: "stable",
    steps: [
      "Connect to WebSocket ws://127.0.0.1:3001 for Gestalt timeline events",
      "Connect to WebSocket ws://127.0.0.1:8080/ws for Atlas ADE live task DAG streaming",
      "Implement graceful fallback and automatic reconnection",
      "Update local task DAG and live event log when frames arrive"
    ]
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

  // Atlas WS State (:8080/ws)
  const [atlasWsStatus, setAtlasWsStatus] = useState<"connected" | "connecting" | "disconnected">("disconnected");
  const [atlasTasks, setAtlasTasks] = useState<AtlasTask[]>(INITIAL_ATLAS_TASKS);
  const [atlasEvents, setAtlasEvents] = useState<AtlasEvent[]>(INITIAL_ATLAS_EVENTS);
  const [atlasLastUpdated, setAtlasLastUpdated] = useState<string | null>(null);
  const [atlasFrameCount, setAtlasFrameCount] = useState<number>(0);

  // Gestalt WS State (:3001)
  const [gestaltWsStatus, setGestaltWsStatus] = useState<"connected" | "connecting" | "disconnected">("disconnected");

  // Real-time WebSocket connection to Atlas ADE (ws://127.0.0.1:8080/ws)
  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimeout: any = null;
    let isMounted = true;

    const connectAtlasWs = () => {
      if (!isMounted) return;
      try {
        setAtlasWsStatus("connecting");
        if (typeof WebSocket === "undefined") {
          setAtlasWsStatus("disconnected");
          return;
        }

        ws = new WebSocket("ws://127.0.0.1:8080/ws");

        ws.onopen = () => {
          if (!isMounted) return;
          setAtlasWsStatus("connected");
          setLogs((prev) => [...prev, "[Atlas WS :8080] Connected to Atlas ADE live stream (/ws)"]);
        };

        ws.onmessage = (event) => {
          if (!isMounted) return;
          try {
            const data: AtlasWsFrame = JSON.parse(event.data);
            if (Array.isArray(data.tasks)) {
              setAtlasTasks(data.tasks);
            }
            if (Array.isArray(data.events)) {
              setAtlasEvents(data.events);
            }
            setAtlasFrameCount((prev) => prev + 1);
            setAtlasLastUpdated(new Date().toLocaleTimeString());
          } catch (err) {
            console.warn("[Atlas WS] Error parsing frame JSON:", err);
          }
        };

        ws.onerror = () => {
          // Handled gracefully via onclose
        };

        ws.onclose = () => {
          if (!isMounted) return;
          setAtlasWsStatus("disconnected");
          reconnectTimeout = setTimeout(connectAtlasWs, 3000);
        };
      } catch {
        if (!isMounted) return;
        setAtlasWsStatus("disconnected");
        reconnectTimeout = setTimeout(connectAtlasWs, 3000);
      }
    };

    connectAtlasWs();

    return () => {
      isMounted = false;
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (ws) {
        ws.onopen = null;
        ws.onmessage = null;
        ws.onerror = null;
        ws.onclose = null;
        ws.close();
      }
    };
  }, []);

  // Real-time WebSocket connection to Gestalt Timeline Server (ws://127.0.0.1:3001)
  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimeout: any = null;
    let isMounted = true;

    const connectGestaltWs = () => {
      if (!isMounted) return;
      try {
        setGestaltWsStatus("connecting");
        if (typeof WebSocket === "undefined") {
          setGestaltWsStatus("disconnected");
          return;
        }

        ws = new WebSocket("ws://127.0.0.1:3001");

        ws.onopen = () => {
          if (!isMounted) return;
          setGestaltWsStatus("connected");
          setLogs((prev) => [...prev, "[Gestalt WS :3001] Connected to real-time timeline bus"]);
        };

        ws.onmessage = (event) => {
          if (!isMounted) return;
          try {
            const data = JSON.parse(event.data);
            const label = data.type || (data.event && data.event.type) || "timeline_event";
            setLogs((prev) => [...prev, `[Gestalt WS :3001] ${label}: ${JSON.stringify(data).slice(0, 80)}`]);
          } catch {
            setLogs((prev) => [...prev, `[Gestalt WS :3001] Raw: ${String(event.data).slice(0, 80)}`]);
          }
        };

        ws.onerror = () => {};

        ws.onclose = () => {
          if (!isMounted) return;
          setGestaltWsStatus("disconnected");
          reconnectTimeout = setTimeout(connectGestaltWs, 3000);
        };
      } catch {
        if (!isMounted) return;
        setGestaltWsStatus("disconnected");
        reconnectTimeout = setTimeout(connectGestaltWs, 3000);
      }
    };

    connectGestaltWs();

    return () => {
      isMounted = false;
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (ws) {
        ws.onopen = null;
        ws.onmessage = null;
        ws.onerror = null;
        ws.onclose = null;
        ws.close();
      }
    };
  }, []);

  useEffect(() => {
    // Initialize WASM Bridge
    wasmBridge.initialize().then((success) => {
      setWasmInited(success);
      if (success) {
        setLogs((prev) => [...prev, "Gestalt WASM Engine initialized successfully."]);
      }
    });

    // Check real health status of Gestalt Bus (:8081)
    const checkBusHealth = async () => {
      try {
        const res = await fetch("http://127.0.0.1:8081/healthz", { signal: AbortSignal.timeout(1000) });
        setStats((prev) => ({ ...prev, isOnline: res.ok }));
      } catch {
        setStats((prev) => ({ ...prev, isOnline: false }));
      }
    };
    checkBusHealth();
    const interval = setInterval(checkBusHealth, 5000);

    const handleOnline = () => setStats((prev) => ({ ...prev, isOnline: true }));
    const handleOffline = () => setStats((prev) => ({ ...prev, isOnline: false }));
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);
    return () => {
      clearInterval(interval);
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

  const handleSimulateWsFrame = () => {
    const nextTasks: AtlasTask[] = [
      {
        id: "task-001",
        session_id: "sess-atlas-root",
        title: "1. Scope: Long-Horizon Session Architecture",
        state: "COMPLETED",
        agent: "agy",
        attempts: 0,
        created_at: 1757500000,
        updated_at: Date.now(),
        deps: []
      },
      {
        id: "task-002",
        session_id: "sess-atlas-root",
        title: "2. Ground in CodeGraph Symbols (Gestalt VFS)",
        state: "IN_PROGRESS",
        agent: "kimi",
        attempts: 1,
        created_at: 1757500100,
        updated_at: Date.now(),
        deps: ["task-001"]
      },
      {
        id: "task-003",
        session_id: "sess-atlas-root",
        title: "3. Execute Multi-Agent Verification Gate",
        state: "READY",
        agent: "claude",
        attempts: 0,
        created_at: 1757500200,
        updated_at: Date.now(),
        deps: ["task-002"]
      },
      {
        id: "task-004",
        session_id: "sess-atlas-root",
        title: "4. Final Zero-Regression Verification & Merge",
        state: "BLOCKED",
        agent: null,
        attempts: 0,
        created_at: Date.now(),
        updated_at: Date.now(),
        deps: ["task-003"]
      }
    ];

    const nextEvents: AtlasEvent[] = [
      {
        id: atlasEvents.length + 1,
        kind: "task_promoted",
        payload: "Task task-002 promoted to IN_PROGRESS via Gestalt VFS",
        idempotency_key: `sim-${Date.now()}`,
        recorded_at: Date.now()
      },
      ...atlasEvents
    ];

    setAtlasTasks(nextTasks);
    setAtlasEvents(nextEvents);
    setAtlasFrameCount((c) => c + 1);
    setAtlasLastUpdated(new Date().toLocaleTimeString());
    setLogs((prev) => [...prev, "[Atlas WS] Simulated frame update applied to DAG state."]);
  };

  const getTaskStateBadge = (state: string) => {
    switch ((state || "").toUpperCase()) {
      case "READY":
        return "bg-emerald-500/20 text-emerald-400 border-emerald-500/30";
      case "IN_PROGRESS":
        return "bg-cyan-500/20 text-cyan-400 border-cyan-500/30";
      case "COMPLETED":
        return "bg-purple-500/20 text-purple-400 border-purple-500/30";
      case "BLOCKED":
        return "bg-amber-500/20 text-amber-400 border-amber-500/30";
      case "FAILED":
        return "bg-red-500/20 text-red-400 border-red-500/30";
      default:
        return "bg-slate-800 text-slate-300 border-slate-700";
    }
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
          <button
            onClick={() => { setActiveTab("atlas"); setSidebarOpen(false); }}
            className={`flex items-center w-full px-4 py-3 rounded-lg text-sm font-medium transition-colors ${activeTab === "atlas" ? "bg-slate-800 text-emerald-400" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-100"}`}
          >
            <Database className="w-5 h-5 mr-3" />
            Atlas ADE (DAG & Forge)
          </button>
        </nav>

        <div className="p-4 border-t border-slate-800 bg-slate-900/30 space-y-2 text-xs">
          <div className="flex items-center justify-between">
            <span className="text-slate-400">WASM Engine</span>
            <span className={`w-2 h-2 rounded-full ${wasminited ? "bg-emerald-500 animate-pulse" : "bg-red-500"}`} />
          </div>
          <div className="flex items-center justify-between">
            <span className="text-slate-400">Gestalt WS (:3001)</span>
            <span className={`w-2 h-2 rounded-full ${gestaltWsStatus === "connected" ? "bg-emerald-500 animate-pulse" : gestaltWsStatus === "connecting" ? "bg-amber-500 animate-pulse" : "bg-slate-600"}`} />
          </div>
          <div className="flex items-center justify-between">
            <span className="text-slate-400">Atlas ADE (:8080)</span>
            <span className={`w-2 h-2 rounded-full ${atlasWsStatus === "connected" ? "bg-emerald-500 animate-pulse" : atlasWsStatus === "connecting" ? "bg-amber-500 animate-pulse" : "bg-slate-600"}`} />
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

          {activeTab === "atlas" && (
            <div className="space-y-6">
              <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 bg-slate-900/60 p-5 rounded-xl border border-slate-800">
                <div>
                  <h2 className="text-xl font-bold text-slate-100 flex items-center gap-2">
                    <Database className="w-5 h-5 text-emerald-400" />
                    Atlas ADE (Agent Development Environment)
                  </h2>
                  <p className="text-sm text-slate-400 mt-1">
                    Gobernanza de tareas de largo plazo, verificación determinista (DoD), pre-issues DAG y Git Forge mínima.
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-3">
                  <div className="flex items-center gap-2 text-xs font-mono bg-slate-950 px-3 py-1.5 rounded-lg border border-slate-800">
                    <span className={`w-2 h-2 rounded-full ${
                      atlasWsStatus === "connected"
                        ? "bg-emerald-400 animate-pulse"
                        : atlasWsStatus === "connecting"
                        ? "bg-amber-400 animate-pulse"
                        : "bg-slate-500"
                    }`} />
                    <span className="text-slate-300">
                      {atlasWsStatus === "connected"
                        ? "Atlas WS Live (:8080/ws)"
                        : atlasWsStatus === "connecting"
                        ? "Atlas WS Connecting (:8080)"
                        : "Atlas WS Offline (Fallback Mode)"}
                    </span>
                  </div>
                  {atlasLastUpdated && (
                    <span className="text-[11px] text-slate-500 font-mono hidden md:inline">
                      Synced: {atlasLastUpdated} (#{atlasFrameCount})
                    </span>
                  )}
                  <button
                    onClick={handleSimulateWsFrame}
                    className="flex items-center text-xs px-2.5 py-1.5 rounded bg-slate-800 hover:bg-slate-700 text-cyan-400 border border-slate-700 transition-colors font-mono"
                    title="Simula la llegada de un frame WebSocket de Atlas con actualización de tareas"
                  >
                    <RefreshCw className="w-3.5 h-3.5 mr-1.5" />
                    Simulate WS Frame
                  </button>
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="bg-slate-900/40 p-4 rounded-xl border border-slate-800">
                  <span className="text-xs font-medium text-slate-400 uppercase tracking-wider block">Gobernanza DAG</span>
                  <span className="text-lg font-bold text-slate-200 mt-1 block font-mono">
                    {atlasTasks.length} Tareas Totales
                  </span>
                  <p className="text-xs text-slate-500 mt-1">
                    {atlasTasks.filter((t) => t.state === "READY").length} Ready ·{" "}
                    {atlasTasks.filter((t) => t.state === "IN_PROGRESS").length} Active ·{" "}
                    {atlasTasks.filter((t) => t.state === "BLOCKED").length} Blocked ·{" "}
                    {atlasTasks.filter((t) => t.state === "COMPLETED").length} Done
                  </p>
                </div>
                <div className="bg-slate-900/40 p-4 rounded-xl border border-slate-800">
                  <span className="text-xs font-medium text-slate-400 uppercase tracking-wider block">Ejecutor Backend</span>
                  <span className="text-lg font-bold text-slate-200 mt-1 block">
                    Gestalt WS: {gestaltWsStatus === "connected" ? "Activo (:3001)" : "Standby"}
                  </span>
                  <p className="text-xs text-slate-500 mt-1">Aislamiento en Git Worktrees, VFS Overlays y resolución de conflictos.</p>
                </div>
                <div className="bg-slate-900/40 p-4 rounded-xl border border-slate-800">
                  <span className="text-xs font-medium text-slate-400 uppercase tracking-wider block">Memoria & Insights</span>
                  <span className="text-lg font-bold text-slate-200 mt-1 block font-mono">
                    {atlasEvents.length} Eventos Recibidos
                  </span>
                  <p className="text-xs text-slate-500 mt-1">Contexto PRE inyectado y streaming de eventos POST continuo.</p>
                </div>
              </div>

              <div className="bg-slate-900/40 rounded-xl border border-slate-800 p-5">
                <div className="flex items-center justify-between border-b border-slate-800 pb-3 mb-4">
                  <h3 className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                    <Compass className="w-4 h-4 text-emerald-400" />
                    Atlas Task DAG (Live Pre-Issues Hierarchy)
                  </h3>
                  <div className="flex items-center gap-2 text-xs font-mono">
                    <span className="text-slate-400">
                      {atlasWsStatus === "connected" ? "Live Stream (:8080)" : "Fallback DAG Mode"}
                    </span>
                    <span className="px-2 py-0.5 rounded bg-slate-950 text-slate-300 border border-slate-800">
                      {atlasTasks.length} nodes
                    </span>
                  </div>
                </div>
                <div className="space-y-2">
                  {atlasTasks.length === 0 ? (
                    <div className="text-center py-6 text-slate-500 text-xs font-mono">
                      No active tasks in DAG. Connect to Atlas serve (:8080/ws) or simulate a frame.
                    </div>
                  ) : (
                    atlasTasks.map((t, idx) => (
                      <div
                        key={t.id || idx}
                        className={`p-3 bg-slate-950 border border-slate-800/80 rounded-lg flex flex-col sm:flex-row sm:items-center justify-between gap-2 ${
                          t.deps && t.deps.length > 0 ? "ml-0 sm:ml-4 border-l-2 border-l-slate-700" : ""
                        }`}
                      >
                        <div className="flex items-center gap-3">
                          <span className={`px-2 py-0.5 rounded text-[11px] font-mono border ${getTaskStateBadge(t.state)}`}>
                            {t.state}
                          </span>
                          <span className="text-sm font-medium text-slate-200">
                            {t.title}
                          </span>
                        </div>
                        <div className="flex items-center gap-2 text-xs font-mono text-slate-400">
                          {t.agent && (
                            <span className="px-2 py-0.5 rounded bg-slate-900 border border-slate-800 text-cyan-400 text-[11px]">
                              @{t.agent}
                            </span>
                          )}
                          {typeof t.attempts === "number" && t.attempts > 0 && (
                            <span className="text-amber-400 text-[11px]">
                              attempts: {t.attempts}
                            </span>
                          )}
                          <span className="text-slate-500 text-[11px]">
                            {t.deps && t.deps.length > 0 ? `deps: ${t.deps.join(", ")}` : "Parent Root (0 deps)"}
                          </span>
                          <span className="text-slate-600 text-[10px]">
                            #{t.id.slice(0, 8)}
                          </span>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <div className="bg-slate-900/40 rounded-xl border border-slate-800 p-5">
                <div className="flex items-center justify-between border-b border-slate-800 pb-3 mb-4">
                  <h3 className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                    <Activity className="w-4 h-4 text-cyan-400" />
                    Atlas Live Console & Pipeline
                  </h3>
                  <div className="flex items-center gap-2 text-xs font-mono text-slate-400">
                    <span className="text-slate-500">ws://127.0.0.1:8080/ws</span>
                    <span className="px-2 py-0.5 rounded bg-slate-950 border border-slate-800 text-slate-300">
                      {atlasEvents.length} frames
                    </span>
                  </div>
                </div>
                <div className="space-y-2 font-mono text-xs text-slate-300 bg-slate-950 p-4 rounded-lg border border-slate-900 max-h-64 overflow-y-auto">
                  <div className="text-slate-500 pb-1">// Atlas Live Event Stream &amp; Bus:</div>
                  {atlasEvents.map((evt, idx) => (
                    <div key={evt.id || idx} className="flex items-start gap-2 border-l border-slate-800 pl-2 py-0.5">
                      <span className="text-emerald-400 font-bold">
                        [{evt.kind || `EVT-${evt.id}`}]
                      </span>
                      <span className="text-slate-300 flex-1">
                        {evt.payload}
                      </span>
                      {evt.recorded_at && (
                        <span className="text-slate-600 text-[10px]">
                          {new Date(evt.recorded_at > 10000000000 ? evt.recorded_at : evt.recorded_at * 1000).toLocaleTimeString()}
                        </span>
                      )}
                    </div>
                  ))}
                  <div className="text-slate-500 pt-2 border-t border-slate-900">// CLI Quick Access:</div>
                  <div className="text-slate-400">atlas start --goal "..." --draft-xavier | atlas watch | atlas verify &lt;task-id&gt;</div>
                </div>
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
