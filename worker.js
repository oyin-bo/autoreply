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
const CACHE_NAME = 'autoreply-models-v2';

let EngineClass = null;
let engine = null;
let engineInitPromise = null;
let engineInitModelId = null;
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
    caches.delete('autoreply-models-v1').catch(() => {});
    const cache = await caches.open(CACHE_NAME);
    const resp = await cache.match(model.url);
    if (!resp) return null;
    const ct = (resp.headers.get('content-type') || '').toLowerCase();
    const cl = Number(resp.headers.get('content-length') || 0);
    if (ct.includes('json') || (cl > 0 && cl < 10_000_000)) {
      await cache.delete(model.url);
      return null;
    }
    return resp;
  } catch {
    return null;
  }
}

async function checkCache(modelId) {
  const model = resolveModel(modelId);
  const resp = await getCachedResponse(model);
  let estimate = { usage: 0, quota: 0 };
  try {
    if (navigator.storage?.estimate) {
      estimate = await navigator.storage.estimate();
    }
  } catch {
    // Ignore estimate errors
  }

  const size = resp
    ? Number(resp.headers.get('content-length')) || model.approxBytes
    : 0;

  return {
    modelId: model.id,
    label: model.label,
    approxSizeLabel: model.approxSizeLabel,
    cached: Boolean(resp),
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
    for (const name of [CACHE_NAME, 'autoreply-models-v1']) {
      if (await caches.delete(name)) removed.push(name);
    }
  }

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
    const mod = await import('https://cdn.jsdelivr.net/npm/@litert-lm/core@0.19.0/+esm');
    EngineClass = mod;
  }

  // @litert-lm/core calls createWasmLib without a fileLocator, so in a Worker
  // Emscripten defaults to self.location's origin unless self.Module.locateFile is set.
  self.Module = {
    locateFile: (file) =>
      /^https?:\/\//.test(file)
        ? file
        : `https://cdn.jsdelivr.net/npm/@litert-lm/core@0.19.0/wasm/${file}`,
  };

  const backendEnum = EngineClass.Backend?.GPU_ARTISAN ?? EngineClass.Backend?.GPU ?? 2;
  engine = await EngineClass.Engine.create({
    model: stream,
    backend: backendEnum,
    mainExecutorSettings: {
      maxNumTokens: 4096,
      backendConfig: {
        num_output_candidates: 1,
        wait_for_weight_uploads: true,
        num_decode_steps_per_sync: 1,
        sequence_batch_size: 0,
        supported_lora_ranks: [],
        max_top_k: 64,
        enable_decode_logits: false,
        enable_external_embeddings: false,
        use_submodel: true,
        use_autosized_ringbuffers: true,
      },
    },
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

  if (typeof caches === 'undefined') {
    throw new Error('Browser Cache API is not available in this context.');
  }

  if (downloadAbortController) {
    downloadAbortController.abort();
  }
  downloadAbortController = new AbortController();

  try {
    const resp = await fetch(model.url, {
      mode: 'cors',
      credentials: 'omit',
      signal: downloadAbortController.signal,
    });

    if (!resp.ok || !resp.body) {
      throw new Error(`HTTP ${resp.status} from ${resp.url || model.url}`);
    }

    const total = Number(resp.headers.get('content-length')) || model.approxBytes;
    const cache = await caches.open(CACHE_NAME);
    const clone = resp.clone();
    const reader = resp.body.getReader();

    let loaded = 0;
    let lastProgressAt = 0;

    self.postMessage({
      type: 'DOWNLOAD_PROGRESS',
      modelId: model.id,
      loaded: 0,
      total,
      percent: 0,
    });

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) {
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
      }
    }

    await cache.put(model.url, clone);

    self.postMessage({
      type: 'DOWNLOAD_PROGRESS',
      modelId: model.id,
      loaded: total,
      total,
      percent: 100,
    });
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size: total,
    });
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
  if (engineInitPromise && engineInitModelId === model.id) {
    return engineInitPromise;
  }

  const resp = await getCachedResponse(model);
  if (!resp) {
    throw new Error(`Model ${model.id} is not cached.`);
  }

  engineInitModelId = model.id;
  engineInitPromise = (async () => {
    try {
      const blob = await resp.blob();
      return await loadEngineFromStream(model, blob.stream());
    } finally {
      engineInitPromise = null;
      engineInitModelId = null;
    }
  })();
  return engineInitPromise;
}

function extractTextFromChunk(rawValue) {
  if (!rawValue) return { text: '', thought: '' };
  let chunk = rawValue;
  if (typeof chunk === 'string') {
    try {
      chunk = JSON.parse(chunk);
    } catch {
      return { text: chunk, thought: '' };
    }
  }

  let text = '';
  if (typeof chunk.content === 'string') {
    text = chunk.content;
  } else if (Array.isArray(chunk.content)) {
    text = chunk.content
      .map((part) => {
        if (typeof part === 'string') return part;
        if (part && typeof part.text === 'string') return part.text;
        return '';
      })
      .join('');
  }

  const thought =
    chunk?.channels && typeof chunk.channels.thought === 'string'
      ? chunk.channels.thought
      : '';

  return { text, thought };
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

  const stream = activeConversation.sendMessageStreaming({
    role: 'user',
    content: [{ type: 'text', text: userPrompt }],
  });
  const reader = stream.getReader();
  let totalEmittedChars = 0;

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

      const { text, thought } = extractTextFromChunk(value);
      if (thought && !text) {
        self.postMessage({
          type: 'GENERATION_THINKING',
          requestId,
        });
      }
      if (text) {
        totalEmittedChars += text.length;
        self.postMessage({
          type: 'GENERATION_CHUNK',
          requestId,
          chunk: text,
        });
      }
    }

    if (activeRequestId === requestId) {
      if (totalEmittedChars === 0) {
        console.warn('[autoreply] Generation finished with 0 output chars');
      }
      self.postMessage({
        type: 'GENERATION_DONE',
        requestId,
        empty: totalEmittedChars === 0,
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
