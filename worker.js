const MODELS = {
  'gemma-4-E2B-it-gpu.litertlm': {
    id: 'gemma-4-E2B-it-gpu.litertlm',
    label: 'Gemma 4 E2B',
    approxSizeLabel: '~2.3 GB',
    approxBytes: 2_300_000_000,
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

let EngineClass = null;
let engine = null;
let loadedModelId = null;
let activeConversation = null;
let activeRequestId = null;
let downloadAbortController = null;

function resolveModel(modelId) {
  return MODELS[modelId] || MODELS[DEFAULT_MODEL_ID];
}

async function getOpfsRoot() {
  if (!navigator.storage || !navigator.storage.getDirectory) {
    throw new Error('Origin Private File System (OPFS) is not supported in this browser.');
  }
  return navigator.storage.getDirectory();
}

async function getCachedModelFile(modelId) {
  const root = await getOpfsRoot();
  try {
    const handle = await root.getFileHandle(modelId);
    const file = await handle.getFile();
    if (file && file.size > 0) {
      return file;
    }
  } catch {
    // Not cached
  }
  return null;
}

async function checkCache(modelId) {
  const model = resolveModel(modelId);
  const file = await getCachedModelFile(model.id);
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
    cached: Boolean(file),
    size: file ? file.size : 0,
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

  const root = await getOpfsRoot();
  const removed = [];
  for await (const [name] of root.entries()) {
    if (name.endsWith('.litertlm') || name.endsWith('.part')) {
      await root.removeEntry(name);
      removed.push(name);
    }
  }
  return removed;
}

async function downloadModel(modelId) {
  const model = resolveModel(modelId);
  const existing = await getCachedModelFile(model.id);
  if (existing) {
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size: existing.size,
    });
    return existing;
  }

  if (downloadAbortController) {
    downloadAbortController.abort();
  }
  downloadAbortController = new AbortController();

  const root = await getOpfsRoot();
  const partName = `${model.id}.part`;
  const partHandle = await root.getFileHandle(partName, { create: true });
  const writable = await partHandle.createWritable();

  try {
    const response = await fetch(model.url, {
      mode: 'cors',
      credentials: 'omit',
      signal: downloadAbortController.signal,
    });

    if (!response.ok || !response.body) {
      throw new Error(`Failed to fetch model (${response.status} ${response.statusText})`);
    }

    const total = Number(response.headers.get('content-length')) || model.approxBytes;
    const reader = response.body.getReader();
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
      await writable.write(value);
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

    await writable.close();

    if (typeof partHandle.move === 'function') {
      await partHandle.move(model.id);
    } else {
      const finalHandle = await root.getFileHandle(model.id, { create: true });
      const finalWritable = await finalHandle.createWritable();
      const partFile = await partHandle.getFile();
      await partFile.stream().pipeTo(finalWritable);
      await root.removeEntry(partName);
    }

    const finalFile = await getCachedModelFile(model.id);
    self.postMessage({
      type: 'DOWNLOAD_PROGRESS',
      modelId: model.id,
      loaded: finalFile?.size || loaded,
      total: finalFile?.size || loaded,
      percent: 100,
    });
    self.postMessage({
      type: 'DOWNLOAD_COMPLETE',
      modelId: model.id,
      size: finalFile?.size || loaded,
    });
    return finalFile;
  } catch (err) {
    try {
      await writable.abort();
    } catch {
      // Ignore abort failure
    }
    try {
      await root.removeEntry(partName);
    } catch {
      // Ignore cleanup failure
    }
    if (err?.name === 'AbortError') {
      self.postMessage({ type: 'DOWNLOAD_CANCELLED', modelId: model.id });
      return null;
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

  if (!navigator.gpu) {
    throw new Error('WebGPU is not available in this browser.');
  }

  const file = await getCachedModelFile(model.id);
  if (!file) {
    throw new Error(`Model ${model.id} is not cached in OPFS.`);
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
    model: file.stream(),
  });
  loadedModelId = model.id;

  self.postMessage({ type: 'ENGINE_READY', modelId: model.id });
  return engine;
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
