#!/usr/bin/env python3
"""
Gestalt Event Bus Push Helper

One-line fire-and-forget event push helper for Python/Hermes agents.
This script has zero external dependencies, using only the Python 3.12 standard library.
It strictly adheres to the fire-and-forget contract: 3-second timeout, never raises,
and always exits with status 0, even if the bus is down.
"""

import argparse
import datetime
import json
import sys
import urllib.request
import urllib.error

def push(agent, event_type, summary, project, state, metadata=None, run_id=None, bus_url="http://127.0.0.1:8081"):
    """
    Pushes a BusEvent to the Gestalt Universal Event Bus (fire-and-forget).

    This function implements a 3-second timeout, never raises an exception,
    and returns None, guaranteeing that failing to push to the bus does not
    disrupt or crash the calling agent's process.
    """
    try:
        # Normalize metadata
        parsed_metadata = None
        if metadata is not None:
            if isinstance(metadata, str):
                try:
                    parsed_metadata = json.loads(metadata)
                except Exception:
                    # Fallback to string wrapper if it's not valid JSON
                    parsed_metadata = {"raw": metadata}
            else:
                parsed_metadata = metadata

        # Construct BusEvent payload
        payload = {
            "agent": agent,
            "event_type": event_type,
            "summary": summary,
            "project": project,
            "state": state,
            "metadata": parsed_metadata,
            "run_id": run_id,
            "ts": datetime.datetime.now(datetime.timezone.utc).isoformat()
        }

        # Send POST request to the local event bus
        url = f"{bus_url.rstrip('/')}/api/event"
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST"
        )

        # Fire-and-forget timeout of 3 seconds
        with urllib.request.urlopen(req, timeout=3) as response:
            _ = response.read()
    except Exception:
        # Fire-and-forget: fail silently, never raise
        pass

def main():
    parser = argparse.ArgumentParser(description="Gestalt Event Bus Push Helper CLI")
    parser.add_argument("--agent", required=True, help="Originating agent name")
    parser.add_argument("--event_type", required=True, help="Type of the event")
    parser.add_argument("--summary", required=True, help="Human-readable event summary")
    parser.add_argument("--project", default=None, help="Associated project name")
    parser.add_argument("--state", default=None, help="Agent state (Pending, Running, Success, etc.)")
    parser.add_argument("--metadata", default=None, help="JSON-formatted metadata string or raw string")
    parser.add_argument("--run_id", default=None, help="Run ID of the event")
    parser.add_argument("--bus-url", default="http://127.0.0.1:8081", help="Event bus URL")

    args = parser.parse_args()

    push(
        agent=args.agent,
        event_type=args.event_type,
        summary=args.summary,
        project=args.project,
        state=args.state,
        metadata=args.metadata,
        run_id=args.run_id,
        bus_url=args.bus_url
    )

if __name__ == "__main__":
    main()
