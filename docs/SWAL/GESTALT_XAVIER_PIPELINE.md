# Gestalt ↔ Xavier Pipeline Endpoint Specification

This document details the canonical HTTP endpoints and CLI interfaces for the Gestalt Event Bus and Xavier integration pipeline.

## Bus Endpoints Table

| Route | HTTP Method | Description | Primary / Alias | Response Example |
|---|---|---|---|---|
| `/health` | `GET` | Health check probe | Alias (`/bus/health`, `/healthz`) | `{"status":"ok","service":"gestalt-bus"}` |
| `/bus/health` | `GET` | Health check probe | Alias (`/health`, `/healthz`) | `{"status":"ok","service":"gestalt-bus"}` |
| `/healthz` | `GET` | Liveness probe | Canonical supervisor probe | `{"status":"ok","service":"gestalt-bus"}` |
| `/api/event` | `POST` | Ingest BusEvent from agents | Primary | `{"status":"ok","seq":49,"deduped":false,"ts":"..."}` |
| `/bus/event` | `POST` | Ingest BusEvent from agents | Alias | `{"status":"ok","seq":50,"deduped":false,"ts":"..."}` |
| `/api/events` | `GET` | Query & tail bus events | Primary | `{"count":47,"events":[...],"next_seq":47,"cursor":47}` |
| `/bus/events` | `GET` | Query & tail bus events | Alias | `{"count":47,"events":[...],"next_seq":47,"cursor":47}` |

## CLI Replay & Diagnostics Commands

- **Event Bus Replay**: Replay unsynced bus events from StateDb to Xavier after an outage:
  ```bash
  gestalt bus replay --after-seq 0 --dry-run
  gestalt bus replay --after-seq 49
  ```
- **Gestalt Doctor**: Environment sanity check including bus reachability:
  ```bash
  gestalt doctor
  ```
