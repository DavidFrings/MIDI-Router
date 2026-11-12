// HTTP + WebSocket client for the router API. Every request carries the PIN in X-Pin.

export class ApiError extends Error {
  /** status 0 = network error / timeout. */
  constructor(status) {
    super(status ? `HTTP ${status}` : 'network error');
    this.status = status;
  }
}

export function createApi(getPin) {
  async function call(method, path, body) {
    let res;
    try {
      res = await fetch(path, {
        method,
        cache: 'no-store',
        // keepalive: a release sent while the tab is being hidden still reaches the router
        keepalive: method === 'POST',
        signal: AbortSignal.timeout ? AbortSignal.timeout(4000) : undefined,
        headers: { 'X-Pin': getPin() ?? '', ...(body ? { 'Content-Type': 'application/json' } : {}) },
        body: body ? JSON.stringify(body) : undefined,
      });
    } catch {
      throw new ApiError(0);
    }
    if (!res.ok) throw new ApiError(res.status);
    return res.status === 204 ? null : res.json();
  }

  // Presses go out strictly in order, so a release can never overtake its press (stuck note).
  let pressChain = Promise.resolve();

  return {
    state: (page) => call('GET', `/api/state?page=${page}`),
    press(page, note, pressed) {
      const p = pressChain.then(() => call('POST', '/api/press', { page, note, pressed }));
      pressChain = p.catch(() => {});
      return p;
    },
    fader: (page, index, value) => call('POST', '/api/fader', { page, index, value }),
    openWs: () => new WebSocket(
      `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws?pin=${encodeURIComponent(getPin() ?? '')}`),
  };
}
