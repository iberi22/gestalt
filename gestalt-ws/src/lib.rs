//! Gestalt WebSocket server — broadcasts timeline events to connected clients.
//!
//! # Architecture
//!
//! ```text
//!   MemState (broadcast::Sender<TimelineEvent>)
//!        │
//!        ▼
//!   WsServer::start()       ◄── tokio::spawn in Router
//!        │
//!        ├── accept loop (TcpListener on port :3001)
//!        │       │
//!        │       └── per‑connection task: subscribe → convert → forward JSON
//!        │
//!        └── shutdown via watch::Sender<bool>
//! ```
//!
//! The server subscribes to [`MemState`]'s broadcast channel and maps
//! recognised [`TimelineEvent`] variants to [`WsEvent`] values:
//!
//! | MemState `event_type` | WsEvent variant       |
//! |------------------------|-----------------------|
//! | `"state_change"`       | `StateChanged`        |
//! | `"lock_acquired"`      | `LockAcquired`        |
//! | `"lock_released"`      | `LockReleased`        |

pub mod event;
pub mod server;

pub use event::WsEvent;
pub use server::WsServer;
