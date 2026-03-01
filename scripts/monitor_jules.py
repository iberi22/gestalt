import os
import time
import requests
import sys

# Configuration from environment variables
API_KEY = os.environ.get("JULES_API_KEY")
REPO_OWNER = os.environ.get("REPO_OWNER", "iberi22")
REPO_NAME = os.environ.get("REPO_NAME", "gestalt-rust")
POLL_INTERVAL = int(os.environ.get("POLL_INTERVAL", "60"))  # Seconds
MAX_RETRIES = int(os.environ.get("MAX_RETRIES", "120"))    # Roughly 2 hours by default

API_BASE_URL = "https://jules.googleapis.com/v1alpha"

def get_active_sessions():
    """Fetches sessions and filters for non-terminal ones belonging to the target repo."""
    headers = {"X-Goog-Api-Key": API_KEY}
    try:
        response = requests.get(f"{API_BASE_URL}/sessions", headers=headers)
        response.raise_for_status()
        data = response.json()

        sessions = data.get("sessions", [])
        active_sessions = []

        for session in sessions:
            # Check if session belongs to this repo
            source_context = session.get("sourceContext", {})
            github_context = source_context.get("gitHubRepoContext", {})

            if (github_context.get("owner") == REPO_OWNER and
                github_context.get("repo") == REPO_NAME):

                state = session.get("state")
                # Terminal states: COMPLETED, FAILED
                if state not in ["COMPLETED", "FAILED"]:
                    active_sessions.append(session)

        return active_sessions
    except Exception as e:
        print(f"Error fetching sessions: {e}")
        return None

def main():
    if not API_KEY:
        print("Error: JULES_API_KEY not found in environment.")
        sys.exit(1)

    print(f"Monitoring Jules sessions for {REPO_OWNER}/{REPO_NAME}...")

    retries = 0
    while retries < MAX_RETRIES:
        active = get_active_sessions()

        if active is None:
            # Error during fetch, retry after a while
            time.sleep(POLL_INTERVAL)
            retries += 1
            continue

        if not active:
            print("No active Jules sessions found for this repository. All tasks are finished.")
            sys.exit(0)

        print(f"Found {len(active)} active Jules sessions. Waiting...")
        for s in active:
            print(f" - Session {s.get('name')} is in state: {s.get('state')}")

        time.sleep(POLL_INTERVAL)
        retries += 1

    print(f"Timed out waiting for Jules sessions after {MAX_RETRIES} polls.")
    sys.exit(1)

if __name__ == "__main__":
    main()
