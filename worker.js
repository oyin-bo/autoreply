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
const CHUNK_SIZE = 32 * 1024 * 1024; // 32 MB per Cache API entry

let EngineClass = null;
let engine = null;
let loadedModelId = null;
let activeConversation = null;
let activeRequestId = null;
let downloadAbortController = null;

function resolveModel(modelId) {
  return MODELS[modelId] || MODELS[DEFAULT_MODEL_ID];
}

function metaKey(model) {
  return `${model.url}#meta`;
}

function chunkKey(model, index) {
  return `${model.url}#chunk-${index}`;
}

async function getCachedModelMeta(model) {
  if (typeof caches === 'undefined') return null;
  try {
    const cache = await caches.open(CACHE_NAME);
    const metaResp = await cache.match(metaKey(model));
    if (!metaResp) return null;
    const meta = await metaResp.json();
    if (!meta || !meta.totalSize || !meta.numChunks) return null;

    // Verify all chunks are present
    for (let i = 0; i < meta.numChunks; i++) {
      const hit = await cache.match(chunkKey(model, i));
      if (!hit) return null;
    }
    return meta;
  } catch {
    return null;
  }
}

async function checkCache(modelId) {
  const model = resolveModel(modelId);
  const meta = await getCachedModelMeta(model);
  let estimate = { usage: 0, quota: 0 };
  try {
    if (navigator.storage?.estimate) {
      estimate = await navigator.storage.estimate();
    }
  } catch {
    // Ignore estimate errors
  }

  return {
    modelId: model.id,
    label: model.label,
    approxSizeLabel: model.approxSizeLabel,
    cached: Boolean(meta),
    size: meta ? meta.totalSize : 0,
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

function createCachedModelStream(model, meta) {
  let index = 0;
  let currentReader = null;
  let cachePromise = caches.open(CACHE_NAME);

  return new ReadableStream({
    async pull(controller) {
      while (true) {
        if (!currentReader) {
          if (index >= meta.numChunks) {
            controller.close();
            return;
          }
          const cache = await cachePromise;
          const resp = await cache.match(chunkKey(model, index));
          if (!resp || !resp.body) {
            controller.error(new Error(`Missing cached chunk ${index} for ${model.id}`));
            return;
          }
          currentReader = resp.body.getReader();
          index += 1;
        }

        const { done, value } = await currentReader.read();
        if (done) {
          currentReader = null;
          continue;
        }
        controller.enqueue(value);
        return;
      }
    },
    async cancel(reason) {
      if (currentReader) {
        await currentReader.cancel(reason);
        currentReader = null;
      }
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

async function fetchWithRetry(url, init, retries = 2) {
  let lastErr = null;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const resp = await fetch(url, init);
      if (resp.ok) return resp;
      lastErr = new Error(`HTTP ${resp.status} ${resp.statusText} for ${url}`);
    } catch (err) {
      if (err?.name === 'AbortError') throw err;
      lastErr = err;
    }
    if (attempt < retries) {
      await new Promise((r) => setTimeout(r, 600 * (attempt + 1)));
    }
  }
  throw lastErr;
}

async function downloadModel(modelId) {
  const model = resolveModel(modelId);
  const existingMeta = await getCachedModelMeta(model);
  if (existingMeta) {
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size: existingMeta.totalSize,
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
  const signal = downloadAbortController.signal;

  try {
    // Probe total file size via Range 0-0 (also resolves CDN URL if redirected)
    const probeResp = await fetchWithRetry(model.url, {
      method: 'GET',
      headers: { Range: 'bytes=0-0' },
      mode: 'cors',
      credentials: 'omit',
      signal,
    });

    const contentRange = probeResp.headers.get('content-range') || '';
    const rangeMatch = contentRange.match(/\/(\d+)$/);
    const totalSize = rangeMatch
      ? Number(rangeMatch[1])
      : Number(probeResp.headers.get('content-length')) || model.approxBytes;
    try {
      await probeResp.body?.cancel();
    } catch {
      // Ignore cancel error
    }

    const numChunks = Math.ceil(totalSize / CHUNK_SIZE);
    const cache = await caches.open(CACHE_NAME);

    // Save metadata upfront (validity still requires all chunks to exist)
    await cache.put(
      metaKey(model),
      new Response(JSON.stringify({ totalSize, chunkSize: CHUNK_SIZE, numChunks }), {
        headers: { 'Content-Type': 'application/json' },
      })
    );

    let loaded = 0;
    let lastProgressAt = 0;

    self.postMessage({
      type: 'DOWNLOAD_PROGRESS',
      modelId: model.id,
      loaded: 0,
      total: totalSize,
      percent: 0,
    });

    for (let i = 0; i < numChunks; i++) {
      if (signal.aborted) {
        throw new DOMException('Aborted', 'AbortError');
      }

      const start = i * CHUNK_SIZE;
      const end = Math.min(totalSize - 1, start + CHUNK_SIZE - 1);
      const expectedLen = end - start + 1;

      const existingChunk = await cache.match(chunkKey(model, i));
      if (existingChunk) {
        const cachedLen = Number(existingChunk.headers.get('content-length')) || expectedLen;
        if (cachedLen === expectedLen) {
          loaded += expectedLen;
          const percent = Math.min(99, Math.round((loaded / totalSize) * 100));
          self.postMessage({
            type: 'DOWNLOAD_PROGRESS',
            modelId: model.id,
            loaded,
            total: totalSize,
            percent,
          });
          continue;
        }
      }

      // Re-fetch from model.url on each chunk so signed CDN URLs stay fresh
      const chunkResp = await fetchWithRetry(model.url, {
        method: 'GET',
        headers: { Range: `bytes=${start}-${end}` },
        mode: 'cors',
        credentials: 'omit',
        signal,
      });

      const reader = chunkResp.body.getReader();
      const buffer = new Uint8Array(expectedLen);
      let offset = 0;

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (offset + value.byteLength > buffer.byteLength) {
          throw new Error(`Chunk ${i} exceeded expected length`);
        }
        buffer.set(value, offset);
        offset += value.byteLength;
        loaded += value.byteLength;

        const now = performance.now();
        if (now - lastProgressAt >= 150) {
          lastProgressAt = now;
          const percent = Math.min(99, Math.round((loaded / totalSize) * 100));
          self.postMessage({
            type: 'DOWNLOAD_PROGRESS',
            modelId: model.id,
            loaded,
            total: totalSize,
            percent,
          });
        }
      }

      if (offset !== expectedLen) {
        throw new Error(`Incomplete chunk ${i}: got ${offset} of ${expectedLen} bytes`);
      }

      await cache.put(
        chunkKey(model, i),
        new Response(buffer, {
          status: 200,
          headers: {
            'Content-Type': 'application/octet-stream',
            'Content-Length': String(expectedLen),
          },
        })
      );
    }

    self.postMessage({
      type: 'DOWNLOAD_PROGRESS',
      modelId: model.id,
      loaded: totalSize,
      total: totalSize,
      percent: 100,
    });
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size: totalSize,
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

  const meta = await getCachedModelMeta(model);
  if (!meta) {
    throw new Error(`Model ${model.id} is not cached.`);
  }

  const stream = createCachedModelStream(model, meta);
  return loadEngineFromStream(model, stream);
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
