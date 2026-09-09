(() => {
  const origin = __AETHER_SESSION_ORIGIN__;
  if (window !== window.top || window.location.origin !== origin) return;

  const secret = __AETHER_SESSION_SECRET__;
  // Capture native fetch before application scripts can replace it. The
  // capability stays in this closure and is sent only to the fixed origin.
  const request = window.fetch.bind(window);
  Object.defineProperty(window, '__AETHER_DESKTOP__', {
    value: Object.freeze({
      async authenticate(clientDeviceId) {
        if (window.location.origin !== origin || typeof clientDeviceId !== 'string') {
          throw new Error('无法验证本机连接，请重新打开 Aether');
        }
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 10000);
        try {
          const response = await request(`${origin}/_gateway/desktop/session`, {
            method: 'POST',
            mode: 'same-origin',
            credentials: 'same-origin',
            redirect: 'error',
            cache: 'no-store',
            signal: controller.signal,
            headers: {
              'X-Aether-Desktop-Session': secret,
              'X-Client-Device-Id': clientDeviceId,
            },
          });
          if (!response.ok) {
            throw new Error('无法建立本机连接，请在客户端设置中重启网关后重试');
          }
          const session = await response.json();
          if (typeof session.access_token !== 'string' || !session.access_token) {
            throw new Error('本机连接返回无效，请重启网关后重试');
          }
          return session;
        } finally {
          clearTimeout(timeout);
        }
      },
    }),
    configurable: false,
    enumerable: false,
    writable: false,
  });
})();
