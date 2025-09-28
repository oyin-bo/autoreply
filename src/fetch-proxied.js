// @ts-check

import * as http from 'http';
import * as https from 'https';
import * as tls from 'tls';
import { URL } from 'url';

/**
 * Detects OS proxy environment variables and returns appropriate fetch implementation
 * @returns {typeof fetch} Custom fetch function with proxy support or native fetch
 */
export default function createProxyAwareFetch() {
  // Check Node.js version
  const nodeVersion = parseInt(process.version.slice(1).split('.')[0]);

  // Get proxy environment variables (case-insensitive)
  const httpProxy = process.env.HTTP_PROXY || process.env.http_proxy;
  const httpsProxy = process.env.HTTPS_PROXY || process.env.https_proxy;
  const noProxy = process.env.NO_PROXY || process.env.no_proxy;
  const allProxy = process.env.ALL_PROXY || process.env.all_proxy;

  // If no proxy vars or Node.js 24+, use built-in fetch (which respects proxy env vars)
  if ((!httpProxy && !httpsProxy && !allProxy) || nodeVersion >= 24) {
    return globalThis.fetch;
  }

  // For Node.js < 24 with proxy vars, implement custom fetch with proxy support
  return /** @type {typeof fetch} */(proxyFetch);

  /**
   * Custom fetch implementation with proxy support for Node.js < 24
   * @param {string | URL} input - The resource to fetch
   * @param {RequestInit} init - Request options
   * @returns {Promise<Response>} Response object
   */
  async function proxyFetch(input, init = {}) {
    const url = new URL(input);
    const isHttps = url.protocol === 'https:';

    // Parse proxy URL
    const proxyUrl = isHttps ? (httpsProxy || allProxy) : (httpProxy || allProxy);

    // Check if URL should bypass proxy (NO_PROXY)
    const shouldBypassProxy = noProxy && noProxy.split(',').some(pattern => {
      const trimmed = pattern.trim();
      return trimmed === '*' ||
        url.hostname === trimmed ||
        url.hostname.endsWith('.' + trimmed);
    });

    if (!proxyUrl || shouldBypassProxy) {
      // No proxy or bypassed, use direct connection
      return makeDirectRequest(url, init, isHttps);
    }

    const proxy = new URL(proxyUrl);
    return makeProxyRequest(url, init, proxy, isHttps);
  };

  /**
   * Make direct HTTP/HTTPS request
   * @param {URL} url
   * @param {RequestInit} init
   * @param {boolean} isHttps
   */
  function makeDirectRequest(url, init, isHttps) {
    const module = isHttps ? https : http;

    return new Promise((resolve, reject) => {
      /** @type {import('http').RequestOptions} */
      const options = {
        hostname: url.hostname,
        port: url.port,
        path: url.pathname + url.search,
        method: init.method || 'GET',
  headers: /** @type {import('http').OutgoingHttpHeaders} */(init.headers || {})
      };

      const req = module.request(options, (res) => {
        /** @type {Buffer[]} */
        const chunks = [];
        res.on('data', chunk => chunks.push(chunk));
        res.on('end', () => {
          const body = Buffer.concat(chunks);
          const response = createResponseObject(res, body);
          resolve(response);
        });
      });

      req.on('error', reject);

      if (init.body) {
        req.write(init.body);
      }
      req.end();
    });
  }

  /**
   * Make HTTP/HTTPS request through proxy
   * @param {URL} url
   * @param {RequestInit} init
   * @param {URL} proxy
   * @param {boolean} isHttps
   */
  function makeProxyRequest(url, init, proxy, isHttps) {
    return new Promise((resolve, reject) => {
      /** @type {import('http').RequestOptions} */
      const proxyOptions = {
        hostname: proxy.hostname,
        port: proxy.port,
        method: isHttps ? 'CONNECT' : (init.method || 'GET'),
        headers: /** @type {import('http').OutgoingHttpHeaders} */(isHttps ? {} : (init.headers || {}))
      };

      if (isHttps) {
        // HTTPS through HTTP proxy (CONNECT method)
        proxyOptions.path = `${url.hostname}:${url.port || 443}`;

        const proxyReq = http.request(proxyOptions);
        proxyReq.on('connect', (res, socket) => {
          if (res.statusCode === 200) {
            const requestOptions = {
              hostname: url.hostname,
              port: url.port || 443,
              method: init.method || 'GET',
              path: url.pathname + url.search,
              headers: /** @type {import('http').OutgoingHttpHeaders} */(init.headers || {}),
              agent: false,
              createConnection: () => tls.connect({
                socket,
                servername: url.hostname
              })
            };

            const req = https.request(requestOptions, (res) => {
              /** @type {Buffer[]} */
              const chunks = [];
              res.on('data', chunk => chunks.push(chunk));
              res.on('end', () => {
                const body = Buffer.concat(chunks);
                const response = createResponseObject(res, body);
                resolve(response);
              });
            });

            req.on('error', reject);
            if (init.body) req.write(init.body);
            req.end();
          } else {
            reject(new Error(`Proxy CONNECT failed: ${res.statusCode}`));
            socket.destroy();
          }
        });
        proxyReq.on('error', reject);
        proxyReq.end();
      } else {
        // HTTP through HTTP proxy
        proxyOptions.path = url.href;
        proxyOptions.headers = /** @type {import('http').OutgoingHttpHeaders} */(init.headers || {});

        const req = http.request(proxyOptions, (res) => {
          /** @type {Buffer[]} */
          const chunks = [];
          res.on('data', chunk => chunks.push(chunk));
          res.on('end', () => {
            const body = Buffer.concat(chunks);
            const response = createResponseObject(res, body);
            resolve(response);
          });
        });

        req.on('error', reject);
        if (init.body) req.write(init.body);
        req.end();
      }
    });
  }

  /**
   * Create a fetch-like Response object
   * @param {import('http').IncomingMessage} res
   * @param {Buffer} body
   */
  function createResponseObject(res, body) {
    return {
      ok: (res.statusCode || 0) >= 200 && (res.statusCode || 0) < 300,
      status: res.statusCode,
      statusText: res.statusMessage,
      headers: new Map(Object.entries(res.headers)),
      url: res.url,
      async text() { return body.toString(); },
      async json() { return JSON.parse(body.toString()); },
      async arrayBuffer() { return body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength); },
  async blob() { return new Blob([new Uint8Array(body)]); }
    };
  }
}
