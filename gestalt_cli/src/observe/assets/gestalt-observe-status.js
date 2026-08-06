// Opencode Plugin template for Gestalt Observe Status
// Automatically reports event status via universal event bus

(function() {
  const url = 'http://127.0.0.1:8081/api/event';
  const payload = {
    agent: 'opencode-plugin',
    event_type: 'agent_state',
    summary: 'Opencode agent status update',
    state: 'Running',
    ts: new Date().toISOString()
  };

  if (typeof fetch === 'function') {
    let signal = null;
    let timeoutId = null;

    if (typeof AbortController === 'function') {
      const controller = new AbortController();
      signal = controller.signal;
      timeoutId = setTimeout(() => controller.abort(), 10000); // 10s timeout
    }

    fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(payload),
      signal: signal
    })
    .then(response => {
      if (timeoutId) clearTimeout(timeoutId);
    })
    .catch(error => {
      // Fail-open: ignore errors so the agent/editor remains unaffected
      if (timeoutId) clearTimeout(timeoutId);
    });
  }
})();
