const MODELS = {
  'gemma-4-E2B-it-gpu.litertlm': {
    id: 'gemma-4-E2B-it-gpu.litertlm',
    label: 'Gemma 4 E2B',
    approxSizeLabel: '~2.3 GB',
    approxBytes: 2_340_870_772,
    url: 'https://huggingface.co/litert-community/gemma-4-E2B-it-litert-lm/resolve/main/gemma-4-E2B-it-gpu.litertlm',
  },
  'gemma-4-E4B-it-gpu.litertlm': {
    id: 'gemma-4-E4B-it-gpu.litertlm',
    label: 'Gemma 4 E4B',
    approxSizeLabel: '~4.5 GB',
    approxBytes: 4_500_000_000,
    url: 'https://huggingface.co/litert-community/gemma-4-E4B-it-litert-lm/resolve/main/gemma-4-E4B-it-gpu.litertlm',
  },
};

const DEFAULT_MODEL_ID = 'gemma-4-E2B-it-gpu.litertlm';
const CACHE_NAME = 'autoreply-models-v1';

let EngineClass = null;
let engine = null;
let loadedModelId = null;
let activeConversation = null;
let activeRequestId = null;
let downloadAbortController = null;

function resolveModel(modelId) {
  return MODELS[modelId] || MODELS[DEFAULT_MODEL_ID];
}

async function getCachedResponse(model) {
  if (typeof caches === 'undefined') return null;
  try {
    const cache = await caches.open(CACHE_NAME);
    const match = await cache.match(model.url);
    return match || null;
  } catch {
    return null;
  }
}

async function checkCache(modelId) {
  const model = resolveModel(modelId);
  const cachedResp = await getCachedResponse(model);
  let estimate = { usage: 0, quota: 0 };
  try {
    if (navigator.storage?.estimate) {
      estimate = await navigator.storage.estimate();
    }
  } catch {
    // Ignore estimate errors
  }

  const size = cachedResp
    ? Number(cachedResp.headers.get('content-length')) || model.approxBytes
    : 0;

  return {
    modelId: model.id,
    label: model.label,
    approxSizeLabel: model.approxSizeLabel,
    cached: Boolean(cachedResp),
    size,
    estimate,
  };
}

async function purgeCache() {
  if (downloadAbortController) {
    downloadAbortController.abort();
    downloadAbortController = null;
  }

  if (activeConversation) {
    try {
      activeConversation.cancel();
      await activeConversation.delete();
    } catch {
      // Ignore cleanup error
    }
    activeConversation = null;
  }

  if (engine) {
    try {
      await engine.delete();
    } catch {
      // Ignore cleanup error
    }
    engine = null;
    loadedModelId = null;
  }

  const removed = [];
  if (typeof caches !== 'undefined') {
    const deleted = await caches.delete(CACHE_NAME);
    if (deleted) removed.push(CACHE_NAME);
  }

  // Also clean up any legacy OPFS entries
  try {
    if (navigator.storage?.getDirectory) {
      const root = await navigator.storage.getDirectory();
      for await (const [name] of root.entries()) {
        if (name.endsWith('.litertlm') || name.endsWith('.part')) {
          await root.removeEntry(name);
          removed.push(name);
        }
      }
    }
  } catch {
    // Ignore OPFS cleanup errors
  }

  return removed;
}

function createProgressStream(body, model, total) {
  let loaded = 0;
  let lastProgressAt = 0;

  self.postMessage({
    type: 'DOWNLOAD_PROGRESS',
    modelId: model.id,
    loaded: 0,
    total,
    percent: 0,
  });

  const reader = body.getReader();
  return new ReadableStream({
    async pull(controller) {
      const { done, value } = await reader.read();
      if (done) {
        self.postMessage({
          type: 'DOWNLOAD_PROGRESS',
          modelId: model.id,
          loaded,
          total: loaded || total,
          percent: 100,
        });
        controller.close();
        return;
      }

      loaded += value.byteLength;
      const now = performance.now();
      if (now - lastProgressAt >= 150) {
        lastProgressAt = now;
        const percent = total > 0 ? Math.min(99, Math.round((loaded / total) * 100)) : 0;
        self.postMessage({
          type: 'DOWNLOAD_PROGRESS',
          modelId: model.id,
          loaded,
          total,
          percent,
        });
      }
      controller.enqueue(value);
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });
}

async function loadEngineFromStream(model, stream) {
  if (!navigator.gpu) {
    throw new Error('WebGPU is not available in this browser.');
  }

  if (activeConversation) {
    try {
      activeConversation.cancel();
      await activeConversation.delete();
    } catch {
      // Ignore
    }
    activeConversation = null;
  }

  if (engine) {
    try {
      await engine.delete();
    } catch {
      // Ignore
    }
    engine = null;
    loadedModelId = null;
  }

  self.postMessage({ type: 'ENGINE_LOADING', modelId: model.id });

  if (!EngineClass) {
    const mod = await import('https://cdn.jsdelivr.net/npm/@litert-lm/core/+esm');
    EngineClass = mod.Engine;
  }

  engine = await EngineClass.create({
    model: stream,
  });
  loadedModelId = model.id;

  self.postMessage({ type: 'ENGINE_READY', modelId: model.id });
  return engine;
}

async function downloadModel(modelId) {
  const model = resolveModel(modelId);
  const existing = await getCachedResponse(model);
  if (existing) {
    const size = Number(existing.headers.get('content-length')) || model.approxBytes;
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size,
    });
    return;
  }

  if (downloadAbortController) {
    downloadAbortController.abort();
  }
  downloadAbortController = new AbortController();

  try {
    // Check if storage quota can hold the model in Cache API
    let canCache = typeof caches !== 'undefined';
    if (canCache && navigator.storage?.estimate) {
      try {
        const est = await navigator.storage.estimate();
        const available = (est.quota || 0) - (est.usage || 0);
        if (est.quota && available < model.approxBytes * 1.02) {
          canCache = false;
        }
      } catch {
        // Ignore estimate errors
      }
    }

    const response = await fetch(model.url, {
      mode: 'cors',
      credentials: 'omit',
      signal: downloadAbortController.signal,
    });

    if (!response.ok || !response.body) {
      throw new Error(`Failed to fetch model (${response.status} ${response.statusText})`);
    }

    const total = Number(response.headers.get('content-length')) || model.approxBytes;
    const progressStream = createProgressStream(response.body, model, total);

    if (canCache) {
      try {
        const cache = await caches.open(CACHE_NAME);
        const headers = new Headers({
          'Content-Type': 'application/octet-stream',
          'Content-Length': String(total),
        });
        await cache.put(model.url, new Response(progressStream, { status: 200, headers }));
        self.postMessage({
          type: 'DOWNLOAD_COMPLETE',
          modelId: model.id,
          size: total,
        });
        return;
      } catch (cacheErr) {
        if (cacheErr?.name !== 'QuotaExceededError' && !/quota/i.test(cacheErr?.message || '')) {
          throw cacheErr;
        }
        // Storage quota exceeded: fall through to direct streaming into WebGPU Engine
        self.postMessage({
          type: 'CACHE_SKIPPED_QUOTA',
          modelId: model.id,
          message: 'Storage quota too small to cache 2.3 GB—streaming directly into WebGPU...',
        });
        const retryResp = await fetch(model.url, {
          mode: 'cors',
          credentials: 'omit',
          signal: downloadAbortController.signal,
        });
        if (!retryResp.ok || !retryResp.body) {
          throw new Error(`Failed to fetch model (${retryResp.status} ${retryResp.statusText})`);
        }
        const retryStream = createProgressStream(retryResp.body, model, total);
        await loadEngineFromStream(model, retryStream);
        return;
      }
    }

    // Quota estimate already showed < model size: stream directly without caching
    self.postMessage({
      type: 'CACHE_SKIPPED_QUOTA',
      modelId: model.id,
      message: 'Browser storage quota < model size—streaming directly into WebGPU...',
    });
    await loadEngineFromStream(model, progressStream);
  } catch (err) {
    if (err?.name === 'AbortError') {
      self.postMessage({ type: 'DOWNLOAD_CANCELLED', modelId: model.id });
      return;
    }
    throw err;
  } finally {
    downloadAbortController = null;
  }
}

async function ensureEngine(modelId) {
  const model = resolveModel(modelId);
  if (engine && loadedModelId === model.id) {
    return engine;
  }

  const cachedResp = await getCachedResponse(model);
  if (!cachedResp || !cachedResp.body) {
    throw new Error(`Model ${model.id} is not cached.`);
  }

  return loadEngineFromStream(model, cachedResp.body);
}

function extractTextFromChunk(chunk) {
  if (!chunk) return '';
  if (typeof chunk.content === 'string') return chunk.content;
  if (Array.isArray(chunk.content)) {
    return chunk.content
      .map((part) => (part && part.type === 'text' ? part.text || '' : ''))
      .join('');
  }
  return '';
}

async function generate({ requestId, modelId, prefaceMessages, userPrompt }) {
  activeRequestId = requestId;

  const currentEngine = await ensureEngine(modelId);

  if (activeRequestId !== requestId) return;

  if (activeConversation) {
    try {
      activeConversation.cancel();
      await activeConversation.delete();
    } catch {
      // Ignore
    }
    activeConversation = null;
  }

  activeConversation = await currentEngine.createConversation({
    preface: {
      messages: prefaceMessages,
    },
  });

  if (activeRequestId !== requestId) return;

  const stream = activeConversation.sendMessageStreaming(userPrompt);
  const reader = stream.getReader();

  try {
    while (true) {
      if (activeRequestId !== requestId) {
        await reader.cancel();
        break;
      }

      const { done, value } = await reader.read();
      if (done) break;
      if (activeRequestId !== requestId) {
        await reader.cancel();
        break;
      }

      const textChunk = extractTextFromChunk(value);
      if (textChunk) {
        self.postMessage({
          type: 'GENERATION_CHUNK',
          requestId,
          chunk: textChunk,
        });
      }
    }

    if (activeRequestId === requestId) {
      self.postMessage({
        type: 'GENERATION_DONE',
        requestId,
      });
    }
  } finally {
    reader.releaseLock();
    if (activeRequestId === requestId) {
      activeRequestId = null;
    }
  }
}

self.addEventListener('error', (event) => {
  self.postMessage({
    type: 'WORKER_ERROR',
    error: event.message || 'Worker runtime error',
    fatal: true,
  });
});

self.addEventListener('unhandledrejection', (event) => {
  const msg = event.reason?.message || String(event.reason || 'Unhandled worker rejection');
  self.postMessage({
    type: 'WORKER_ERROR',
    error: msg,
    fatal: /device|lost|webgpu|out of memory/i.test(msg),
  });
});

self.addEventListener('message', async (event) => {
  const msg = event.data;
  if (!msg || !msg.type) return;

  try {
    switch (msg.type) {
      case 'CHECK_CACHE': {
        const info = await checkCache(msg.modelId);
        self.postMessage({ type: 'CACHE_STATUS', ...info });
        break;
      }

      case 'DOWNLOAD_MODEL': {
        await downloadModel(msg.modelId);
        break;
      }

      case 'CANCEL_DOWNLOAD': {
        if (downloadAbortController) {
          downloadAbortController.abort();
        }
        break;
      }

      case 'PURGE_CACHE': {
        const removed = await purgeCache();
        self.postMessage({ type: 'CACHE_PURGED', removed });
        const info = await checkCache(msg.modelId);
        self.postMessage({ type: 'CACHE_STATUS', ...info });
        break;
      }

      case 'INIT_ENGINE': {
        await ensureEngine(msg.modelId);
        break;
      }

      case 'GENERATE': {
        await generate(msg);
        break;
      }

      case 'CANCEL': {
        if (!msg.requestId || msg.requestId === activeRequestId) {
          activeRequestId = null;
          if (activeConversation) {
            try {
              activeConversation.cancel();
            } catch {
              // Ignore
            }
          }
          self.postMessage({ type: 'GENERATION_CANCELLED', requestId: msg.requestId });
        }
        break;
      }

      default:
        break;
    }
  } catch (err) {
    const errMsg = err?.message || String(err);
    self.postMessage({
      type: 'WORKER_ERROR',
      requestId: msg.requestId,
      error: errMsg,
      fatal: /device|lost|webgpu|wasm|memory/i.test(errMsg),
    });
  }
});

self.postMessage({ type: 'WORKER_BOOTED' });
