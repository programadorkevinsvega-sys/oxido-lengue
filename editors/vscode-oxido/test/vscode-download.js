const RETRYABLE_NETWORK_CODES = new Set([
  'ECONNABORTED',
  'ECONNREFUSED',
  'ECONNRESET',
  'EAI_AGAIN',
  'EHOSTUNREACH',
  'ENETUNREACH',
  'ETIMEDOUT',
  'ERR_SOCKET_CONNECTION_TIMEOUT',
  'UND_ERR_CONNECT_TIMEOUT',
  'UND_ERR_HEADERS_TIMEOUT'
]);

function isRetryableNetworkError(error) {
  const pending = [error];
  const visited = new Set();
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || typeof current !== 'object' || visited.has(current)) continue;
    visited.add(current);
    if (RETRYABLE_NETWORK_CODES.has(current.code)) return true;
    if (Array.isArray(current.errors)) pending.push(...current.errors);
    if (current.cause) pending.push(current.cause);
  }
  return false;
}

function wait(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

async function downloadVSCodeWithRetry(download, { attempts = 3, delayMs = 2000, onRetry, waitFor = wait }) {
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await download();
    } catch (error) {
      if (!isRetryableNetworkError(error) || attempt === attempts) throw error;
      onRetry(attempt + 1, attempts);
      await waitFor(delayMs * attempt);
    }
  }
  throw new Error('descarga de VS Code agotó los intentos sin resultado');
}

module.exports = { downloadVSCodeWithRetry };
