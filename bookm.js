(() => {
  const SCRIPT_URL = new URL(
    (document.currentScript && document.currentScript.src) || 'https://autoreply.ooo/bookm.js'
  );
  const BASE_URL = new URL('.', SCRIPT_URL).href.replace(/\/$/, '');
  const IFRAME_ORIGIN = SCRIPT_URL.origin;
  const DEFAULT_MODEL =
    SCRIPT_URL.searchParams.get('model') ||
    localStorage.getItem('__autoreply_model') ||
    'gemma-4-E2B-it-gpu.litertlm';

  const RECENT_POSTS_KEY = '__autoreply_recent_posts';
  const HESITATION_MS = 1650;
  const MAX_POST_CHARS = 300;

  if (window.__autoreply) {
    window.__autoreply.reactivate();
    return;
  }

  const state = {
    modelId: DEFAULT_MODEL,
    enabled: true,
    status: 'booting', // booting | consent | downloading | loading | ready | generating | error | paused
    statusDetail: 'Starting...',
    cached: false,
    downloadPercent: 0,
    downloadLoaded: 0,
    downloadTotal: 0,
    drawerOpen: false,
    activeComposer: null,
    debounceTimer: null,
    activeRequestId: null,
    requestCounter: 0,
    suggestionText: '',
    suggestionDraft: '',
    isStreaming: false,
    lastContextPayload: null,
  };

  // --- Hidden Cross-Origin Iframe ---
  const iframe = document.createElement('iframe');
  iframe.src = `${BASE_URL}/iframe.html?model=${encodeURIComponent(state.modelId)}`;
  iframe.allow = 'cross-origin-isolated';
  iframe.style.cssText =
    'position:fixed;width:0;height:0;border:0;opacity:0;pointer-events:none;';
  document.documentElement.appendChild(iframe);

  function postToWorker(msg) {
    if (!iframe.contentWindow) return;
    iframe.contentWindow.postMessage({ source: 'autoreply-host', ...msg }, IFRAME_ORIGIN);
  }

  // --- Shadow DOM UI ---
  const host = document.createElement('div');
  host.id = 'autoreply-root';
  host.style.cssText =
    'position:fixed;top:0;left:0;width:0;height:0;z-index:2147483647;pointer-events:none;';
  document.documentElement.appendChild(host);

  const shadow = host.attachShadow({ mode: 'open' });
  shadow.innerHTML = `
    <style>
      :host {
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
        color: #e2e8f0;
      }
      * { box-sizing: border-box; }
      .ar-ghost {
        position: fixed;
        pointer-events: none;
        color: #8fa6bc;
        white-space: pre-wrap;
        word-break: break-word;
        overflow-wrap: break-word;
        z-index: 2147483645;
        display: none;
      }
      .ar-actions {
        position: fixed;
        pointer-events: auto;
        display: none;
        align-items: center;
        gap: 6px;
        padding: 4px 8px;
        border-radius: 999px;
        background: rgba(15, 23, 42, 0.92);
        border: 1px solid rgba(148, 163, 184, 0.28);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
        backdrop-filter: blur(10px);
        font-size: 12px;
        z-index: 2147483646;
      }
      .ar-btn {
        appearance: none;
        border: 0;
        background: rgba(255, 255, 255, 0.08);
        color: #e2e8f0;
        border-radius: 999px;
        padding: 3px 9px;
        font-size: 11.5px;
        font-weight: 500;
        cursor: pointer;
        display: inline-flex;
        align-items: center;
        gap: 4px;
        transition: background 0.15s ease;
      }
      .ar-btn:hover { background: rgba(255, 255, 255, 0.18); }
      .ar-btn-primary {
        background: #2563eb;
        color: #ffffff;
      }
      .ar-btn-primary:hover { background: #1d4ed8; }
      .ar-kbd {
        font-size: 10px;
        opacity: 0.75;
        padding: 1px 4px;
        border-radius: 4px;
        background: rgba(0, 0, 0, 0.25);
      }
      .ar-pill-wrap {
        position: fixed;
        bottom: 16px;
        right: 16px;
        pointer-events: auto;
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 8px;
        z-index: 2147483647;
      }
      .ar-pill {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 7px 12px;
        border-radius: 999px;
        background: rgba(15, 23, 42, 0.92);
        border: 1px solid rgba(148, 163, 184, 0.28);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
        backdrop-filter: blur(12px);
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        user-select: none;
      }
      .ar-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: #64748b;
      }
      .ar-dot.ready { background: #22c55e; }
      .ar-dot.working { background: #3b82f6; animation: arPulse 1.1s infinite ease-in-out; }
      .ar-dot.error { background: #ef4444; }
      .ar-dot.consent { background: #f59e0b; }
      @keyframes arPulse {
        0%, 100% { opacity: 1; transform: scale(1); }
        50% { opacity: 0.45; transform: scale(0.85); }
      }
      .ar-card {
        width: 310px;
        padding: 14px;
        border-radius: 14px;
        background: rgba(15, 23, 42, 0.96);
        border: 1px solid rgba(148, 163, 184, 0.28);
        box-shadow: 0 16px 40px rgba(0, 0, 0, 0.48);
        backdrop-filter: blur(16px);
        font-size: 12.5px;
        line-height: 1.45;
        display: none;
      }
      .ar-card.open { display: block; }
      .ar-card-title {
        font-size: 13.5px;
        font-weight: 600;
        margin-bottom: 6px;
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      .ar-muted { color: #94a3b8; font-size: 11.5px; }
      .ar-progress-track {
        width: 100%;
        height: 6px;
        border-radius: 999px;
        background: rgba(148, 163, 184, 0.2);
        overflow: hidden;
        margin: 10px 0 6px;
      }
      .ar-progress-fill {
        height: 100%;
        width: 0%;
        background: linear-gradient(90deg, #3b82f6, #06b6d4);
        transition: width 0.15s ease;
      }
      .ar-row {
        display: flex;
        gap: 8px;
        margin-top: 10px;
        flex-wrap: wrap;
      }
      .ar-select {
        width: 100%;
        margin-top: 8px;
        padding: 6px 8px;
        border-radius: 8px;
        background: rgba(30, 41, 59, 0.9);
        color: #e2e8f0;
        border: 1px solid rgba(148, 163, 184, 0.28);
        font-size: 12px;
      }
    </style>

    <div class="ar-ghost" id="ar-ghost"></div>

    <div class="ar-actions" id="ar-actions">
      <button class="ar-btn ar-btn-primary" id="ar-accept">Accept <span class="ar-kbd">Tab</span></button>
      <button class="ar-btn" id="ar-retry" title="Retry">↻ Retry</button>
      <button class="ar-btn" id="ar-dismiss" title="Dismiss (Esc)">✕</button>
    </div>

    <div class="ar-pill-wrap">
      <div class="ar-card" id="ar-consent-card">
        <div class="ar-card-title">
          <span>✉️ Enable Local Gemma 4</span>
          <button class="ar-btn" id="ar-consent-close" style="padding:2px 7px;">✕</button>
        </div>
        <div class="ar-muted" id="ar-consent-copy">
          Download and cache Gemma 4 web weights (~2.3 GB) in browser storage (OPFS). Runs 100% locally via WebGPU—no posts leave your device.
        </div>
        <div id="ar-download-ui" style="display:none;">
          <div class="ar-progress-track"><div class="ar-progress-fill" id="ar-progress-fill"></div></div>
          <div class="ar-muted" id="ar-progress-label">Preparing download...</div>
        </div>
        <div class="ar-row" id="ar-consent-actions">
          <button class="ar-btn ar-btn-primary" id="ar-consent-start">Download &amp; Enable</button>
          <button class="ar-btn" id="ar-consent-later">Later</button>
        </div>
      </div>

      <div class="ar-card" id="ar-drawer">
        <div class="ar-card-title">
          <span>✉️ autoreply.ooo</span>
          <button class="ar-btn" id="ar-drawer-close" style="padding:2px 7px;">✕</button>
        </div>
        <div class="ar-muted" id="ar-drawer-status">Initializing...</div>
        <select class="ar-select" id="ar-model-select">
          <option value="gemma-4-E2B-it-gpu.litertlm">Gemma 4 E2B (~2.3 GB, Default)</option>
          <option value="gemma-4-E4B-it-gpu.litertlm">Gemma 4 E4B (~4.5 GB)</option>
        </select>
        <div class="ar-row">
          <button class="ar-btn" id="ar-toggle-enable">Pause</button>
          <button class="ar-btn" id="ar-restart-btn">Restart Engine</button>
          <button class="ar-btn" id="ar-purge-btn">Purge Cache</button>
        </div>
      </div>

      <div class="ar-pill" id="ar-pill">
        <span class="ar-dot" id="ar-dot"></span>
        <span id="ar-pill-text">autoreply</span>
      </div>
    </div>
  `;

  const elGhost = shadow.getElementById('ar-ghost');
  const elActions = shadow.getElementById('ar-actions');
  const elAccept = shadow.getElementById('ar-accept');
  const elRetry = shadow.getElementById('ar-retry');
  const elDismiss = shadow.getElementById('ar-dismiss');
  const elPill = shadow.getElementById('ar-pill');
  const elDot = shadow.getElementById('ar-dot');
  const elPillText = shadow.getElementById('ar-pill-text');
  const elDrawer = shadow.getElementById('ar-drawer');
  const elDrawerClose = shadow.getElementById('ar-drawer-close');
  const elDrawerStatus = shadow.getElementById('ar-drawer-status');
  const elModelSelect = shadow.getElementById('ar-model-select');
  const elToggleEnable = shadow.getElementById('ar-toggle-enable');
  const elRestartBtn = shadow.getElementById('ar-restart-btn');
  const elPurgeBtn = shadow.getElementById('ar-purge-btn');
  const elConsentCard = shadow.getElementById('ar-consent-card');
  const elConsentCopy = shadow.getElementById('ar-consent-copy');
  const elConsentClose = shadow.getElementById('ar-consent-close');
  const elConsentStart = shadow.getElementById('ar-consent-start');
  const elConsentLater = shadow.getElementById('ar-consent-later');
  const elConsentActions = shadow.getElementById('ar-consent-actions');
  const elDownloadUi = shadow.getElementById('ar-download-ui');
  const elProgressFill = shadow.getElementById('ar-progress-fill');
  const elProgressLabel = shadow.getElementById('ar-progress-label');

  elModelSelect.value = state.modelId;

  function formatBytes(bytes) {
    if (!bytes) return '0 MB';
    const gb = bytes / 1_000_000_000;
    if (gb >= 1) return `${gb.toFixed(2)} GB`;
    return `${(bytes / 1_000_000).toFixed(0)} MB`;
  }

  function renderStatus() {
    elDot.className = 'ar-dot';
    if (!state.enabled) {
      elPillText.textContent = 'autoreply · Paused';
      elDrawerStatus.textContent = 'Auto-suggest is paused.';
      elToggleEnable.textContent = 'Resume';
      return;
    }

    elToggleEnable.textContent = 'Pause';
    switch (state.status) {
      case 'ready':
        elDot.classList.add('ready');
        elPillText.textContent = 'autoreply · Ready';
        break;
      case 'generating':
        elDot.classList.add('working');
        elPillText.textContent = 'autoreply · Writing...';
        break;
      case 'loading':
        elDot.classList.add('working');
        elPillText.textContent = 'autoreply · Loading GPU';
        break;
      case 'downloading':
        elDot.classList.add('working');
        elPillText.textContent = `autoreply · ${state.downloadPercent}%`;
        break;
      case 'consent':
        elDot.classList.add('consent');
        elPillText.textContent = 'autoreply · Setup';
        break;
      case 'error':
        elDot.classList.add('error');
        elPillText.textContent = 'autoreply · Error';
        break;
      default:
        elPillText.textContent = 'autoreply';
        break;
    }
    elDrawerStatus.textContent = state.statusDetail;
  }

  function showConsentCard(show) {
    elConsentCard.classList.toggle('open', show);
    const sizeText = state.modelId.includes('E4B') ? '~4.5 GB' : '~2.3 GB';
    elConsentCopy.textContent = `Download and cache ${state.modelId} (${sizeText}) locally in browser storage (OPFS). Inference runs 100% on-device via WebGPU.`;
  }

  // --- UI Event Handlers ---
  elPill.addEventListener('click', () => {
    state.drawerOpen = !state.drawerOpen;
    elDrawer.classList.toggle('open', state.drawerOpen);
  });

  elDrawerClose.addEventListener('click', () => {
    state.drawerOpen = false;
    elDrawer.classList.remove('open');
  });

  elConsentClose.addEventListener('click', () => showConsentCard(false));
  elConsentLater.addEventListener('click', () => showConsentCard(false));

  elConsentStart.addEventListener('click', () => {
    state.status = 'downloading';
    state.statusDetail = 'Starting download...';
    elDownloadUi.style.display = 'block';
    elConsentActions.innerHTML = '';
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'ar-btn';
    cancelBtn.textContent = 'Cancel Download';
    cancelBtn.addEventListener('click', () => postToWorker({ type: 'CANCEL_DOWNLOAD' }));
    elConsentActions.appendChild(cancelBtn);
    renderStatus();
    postToWorker({ type: 'DOWNLOAD_MODEL', modelId: state.modelId });
  });

  elModelSelect.addEventListener('change', () => {
    state.modelId = elModelSelect.value;
    localStorage.setItem('__autoreply_model', state.modelId);
    state.cached = false;
    clearSuggestion();
    postToWorker({ type: 'CHECK_CACHE', modelId: state.modelId });
  });

  elToggleEnable.addEventListener('click', () => {
    state.enabled = !state.enabled;
    if (!state.enabled) {
      cancelGeneration();
      clearSuggestion();
    }
    renderStatus();
  });

  elRestartBtn.addEventListener('click', () => {
    cancelGeneration();
    clearSuggestion();
    state.status = 'booting';
    state.statusDetail = 'Restarting worker...';
    renderStatus();
    postToWorker({ type: 'RESTART_WORKER' });
  });

  elPurgeBtn.addEventListener('click', () => {
    cancelGeneration();
    clearSuggestion();
    state.cached = false;
    state.status = 'consent';
    state.statusDetail = 'Cache purged.';
    renderStatus();
    postToWorker({ type: 'PURGE_CACHE', modelId: state.modelId });
  });

  // --- Ghost Text & Composer Helpers ---
  function getComposerText(composer) {
    if (!composer) return '';
    return (composer.innerText || composer.textContent || '').replace(/\u200B/g, '');
  }

  function isCaretAtEnd(composer) {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !sel.isCollapsed) return false;
    const range = sel.getRangeAt(0);
    if (!composer.contains(range.startContainer)) return false;

    const afterRange = document.createRange();
    afterRange.selectNodeContents(composer);
    afterRange.setStart(range.endContainer, range.endOffset);
    const trailing = afterRange.toString().replace(/[\s\u200B]/g, '');
    return trailing.length === 0;
  }

  function positionGhostOverlay() {
    const composer = state.activeComposer;
    if (!composer || !state.suggestionText || !document.contains(composer)) {
      elGhost.style.display = 'none';
      elActions.style.display = 'none';
      return;
    }

    const composerRect = composer.getBoundingClientRect();
    if (composerRect.width === 0 || composerRect.height === 0) {
      elGhost.style.display = 'none';
      elActions.style.display = 'none';
      return;
    }

    const sel = window.getSelection();
    let caretRect = null;
    if (sel && sel.rangeCount > 0 && composer.contains(sel.anchorNode)) {
      const range = sel.getRangeAt(0).cloneRange();
      range.collapse(true);
      const rects = range.getClientRects();
      caretRect = rects[0] || range.getBoundingClientRect();
    }

    const cs = window.getComputedStyle(composer);
    const hasValidCaret = caretRect && (caretRect.left > 0 || caretRect.top > 0);
    const topPos = hasValidCaret ? caretRect.top : composerRect.top;
    const indentPx = hasValidCaret ? Math.max(0, caretRect.left - composerRect.left) : 0;

    elGhost.style.left = `${composerRect.left}px`;
    elGhost.style.top = `${topPos}px`;
    elGhost.style.width = `${composerRect.width}px`;
    elGhost.style.fontFamily = cs.fontFamily;
    elGhost.style.fontSize = cs.fontSize;
    elGhost.style.fontWeight = cs.fontWeight;
    elGhost.style.lineHeight = cs.lineHeight;
    elGhost.style.letterSpacing = cs.letterSpacing;
    elGhost.style.textIndent = `${indentPx}px`;
    elGhost.textContent = cleanSuggestionForDisplay(state.suggestionDraft, state.suggestionText);
    elGhost.style.display = 'block';

    const actionTop = Math.min(window.innerHeight - 44, composerRect.bottom + 8);
    const actionLeft = Math.max(8, Math.min(window.innerWidth - 230, composerRect.left));
    elActions.style.top = `${actionTop}px`;
    elActions.style.left = `${actionLeft}px`;
    elActions.style.display = 'flex';
  }

  function cleanSuggestionForDisplay(draft, raw) {
    let text = raw.replace(/^["'“”]+|["'“”]+$/g, '');
    if (draft) {
      // Strip accidental echo of the end of the user's draft
      const trimmedDraft = draft.trimEnd();
      if (text.toLowerCase().startsWith(trimmedDraft.toLowerCase())) {
        text = text.slice(trimmedDraft.length);
      } else {
        const lastWords = trimmedDraft.split(/\s+/).slice(-4).join(' ');
        if (lastWords.length > 3 && text.toLowerCase().startsWith(lastWords.toLowerCase())) {
          text = text.slice(lastWords.length);
        }
      }
      if (
        draft.length > 0 &&
        !/\s$/.test(draft) &&
        text.length > 0 &&
        !/^[\s.,!?;:'")\]\-]/.test(text)
      ) {
        text = ' ' + text;
      }
    } else {
      text = text.trimStart();
    }

    const remainingBudget = Math.max(0, MAX_POST_CHARS - draft.length);
    if (text.length > remainingBudget) {
      text = text.slice(0, remainingBudget);
    }
    return text;
  }

  function clearSuggestion() {
    state.suggestionText = '';
    state.suggestionDraft = '';
    state.isStreaming = false;
    elGhost.style.display = 'none';
    elActions.style.display = 'none';
  }

  function acceptSuggestion() {
    const composer = state.activeComposer;
    if (!composer || !state.suggestionText) return;

    const textToInsert = cleanSuggestionForDisplay(state.suggestionDraft, state.suggestionText);
    clearSuggestion();

    if (!textToInsert) return;

    composer.focus();
    const sel = window.getSelection();
    if (sel) {
      const range = document.createRange();
      range.selectNodeContents(composer);
      range.collapse(false);
      sel.removeAllRanges();
      sel.addRange(range);
    }

    document.execCommand('insertText', false, textToInsert);
  }

  elAccept.addEventListener('click', (e) => {
    e.preventDefault();
    acceptSuggestion();
  });

  elRetry.addEventListener('click', (e) => {
    e.preventDefault();
    if (!state.activeComposer) return;
    triggerGeneration(state.activeComposer, true);
  });

  elDismiss.addEventListener('click', (e) => {
    e.preventDefault();
    cancelGeneration();
    clearSuggestion();
  });

  // --- Bluesky DOM Scraping & Persona Framing ---
  function getCurrentUser() {
    const profileLink =
      document.querySelector('a[data-testid="profileBtn"]') ||
      document.querySelector('nav a[href^="/profile/"]') ||
      document.querySelector('a[aria-label="Profile"][href^="/profile/"]');

    let handle = '';
    if (profileLink) {
      const href = profileLink.getAttribute('href') || '';
      const match = href.match(/^\/profile\/([^/?#]+)/);
      if (match) handle = match[1].toLowerCase();
    }

    if (!handle) {
      try {
        const raw = localStorage.getItem('BSKY_STORAGE');
        if (raw) {
          const parsed = JSON.parse(raw);
          const sess = parsed?.session?.currentAccount;
          if (sess?.handle) handle = sess.handle.toLowerCase();
        }
      } catch {
        // Ignore storage errors
      }
    }

    return { handle };
  }

  function getRecentPosts() {
    try {
      const raw = localStorage.getItem(RECENT_POSTS_KEY);
      const list = raw ? JSON.parse(raw) : [];
      return Array.isArray(list) ? list : [];
    } catch {
      return [];
    }
  }

  function saveRecentPost(text) {
    const clean = (text || '').trim();
    if (!clean || clean.length < 8) return;
    const existing = getRecentPosts().filter((item) => item !== clean);
    existing.unshift(clean);
    try {
      localStorage.setItem(RECENT_POSTS_KEY, JSON.stringify(existing.slice(0, 6)));
    } catch {
      // Ignore quota errors
    }
  }

  function extractPostFromElement(el, currentHandle) {
    const textEl = el.querySelector('[data-testid="postText"]');
    const text = (textEl ? textEl.innerText : '').trim();
    if (!text) return null;

    const authorLink = el.querySelector('a[href^="/profile/"]');
    const href = authorLink?.getAttribute('href') || '';
    const match = href.match(/^\/profile\/([^/?#]+)/);
    const authorHandle = match ? match[1].toLowerCase() : '';

    return {
      role: currentHandle && authorHandle === currentHandle ? 'assistant' : 'user',
      authorHandle,
      content: text,
    };
  }

  function scrapeThreadContext(composer) {
    const { handle: currentHandle } = getCurrentUser();
    const modal = composer.closest('[role="dialog"]');
    const scope = modal || document;

    const candidates = Array.from(
      scope.querySelectorAll('[data-testid^="postThreadItem"], [data-testid="feedItem"]')
    );

    const rawPosts = [];
    for (const el of candidates) {
      if (el.contains(composer)) continue;
      // Only include posts that appear before the composer in DOM order
      const pos = el.compareDocumentPosition(composer);
      if (!(pos & Node.DOCUMENT_POSITION_FOLLOWING)) continue;

      const post = extractPostFromElement(el, currentHandle);
      if (post) {
        if (post.role === 'assistant') {
          saveRecentPost(post.content);
        }
        rawPosts.push(post);
      }
    }

    // In a reply modal, Bluesky sometimes renders the parent post without postThreadItem testid
    if (modal && rawPosts.length === 0) {
      const modalTexts = Array.from(modal.querySelectorAll('[data-testid="postText"]'));
      for (const textEl of modalTexts) {
        if (composer.contains(textEl)) continue;
        const pos = textEl.compareDocumentPosition(composer);
        if (!(pos & Node.DOCUMENT_POSITION_FOLLOWING)) continue;
        const container = textEl.closest('div')?.parentElement || modal;
        const post = extractPostFromElement(container, currentHandle);
        if (post) rawPosts.push(post);
      }
    }

    // Merge consecutive posts by the same role so alternation holds
    const merged = [];
    for (const p of rawPosts.slice(-8)) {
      const prev = merged[merged.length - 1];
      if (prev && prev.role === p.role) {
        prev.content += '\n\n' + p.content;
      } else {
        merged.push({ role: p.role, content: p.content });
      }
    }
    return merged;
  }

  function buildPromptPayload(threadMessages, draft, isRetry) {
    const systemPrompt = [
      'You are co-writing a Bluesky post in the exact voice, tone, and style of the assistant.',
      'Strictly stay under 300 characters total.',
      'Output ONLY the post text—no quotes, no explanations, no meta commentary.',
      draft
        ? 'When the user asks "What was that?", output ONLY the remaining words that directly continue the incomplete thought inside <thinking> without repeating what was already written.'
        : 'Match the user/assistant conversation style naturally and concisely.',
      isRetry ? 'Provide a fresh, alternative phrasing.' : '',
    ]
      .filter(Boolean)
      .join(' ');

    const messages = [{ role: 'system', content: systemPrompt }];

    // Helper to keep strict user/assistant alternation after system
    function pushAlternating(role, content) {
      const prev = messages[messages.length - 1];
      if (prev && prev.role === role) {
        prev.content += '\n\n' + content;
      } else {
        messages.push({ role, content });
      }
    }

    // Case 1: Pre-typed partial draft
    if (draft.trim().length > 0) {
      for (const m of threadMessages) pushAlternating(m.role, m.content);
      if (messages[messages.length - 1]?.role === 'assistant') {
        pushAlternating('user', 'Go on.');
      }
      pushAlternating('assistant', `<thinking> ${draft} </thinking>`);
      return {
        prefaceMessages: messages,
        userPrompt: 'What was that?',
      };
    }

    // Case 2: Brand-new post (empty draft, no parent thread)
    if (threadMessages.length === 0) {
      const recent = getRecentPosts().slice(0, 2);
      if (recent.length > 0) {
        recent.reverse().forEach((postText, idx) => {
          pushAlternating(
            'user',
            idx === 0 ? 'What have you been posting about lately?' : 'Anything else?'
          );
          pushAlternating('assistant', postText);
        });
        pushAlternating('user', 'Nice.');
      }
      pushAlternating('assistant', 'Anyway I want to post another thread.');
      return {
        prefaceMessages: messages,
        userPrompt: 'Go on.',
      };
    }

    // Case 3: Replying in an existing thread (empty draft)
    const lastMsg = threadMessages[threadMessages.length - 1];
    if (lastMsg.role === 'user') {
      for (const m of threadMessages.slice(0, -1)) pushAlternating(m.role, m.content);
      return {
        prefaceMessages: messages,
        userPrompt: lastMsg.content,
      };
    }

    // Last post in thread is by the current user (represented as assistant)
    for (const m of threadMessages) pushAlternating(m.role, m.content);
    return {
      prefaceMessages: messages,
      userPrompt: 'so?',
    };
  }

  // --- Generation Lifecycle ---
  function cancelGeneration() {
    if (state.activeRequestId) {
      postToWorker({ type: 'CANCEL', requestId: state.activeRequestId });
      state.activeRequestId = null;
    }
    state.isStreaming = false;
    if (state.status === 'generating') {
      state.status = state.cached ? 'ready' : state.status;
      renderStatus();
    }
  }

  function triggerGeneration(composer, isRetry = false) {
    if (!state.enabled || !state.cached) return;
    if (state.status === 'downloading' || state.status === 'error') return;
    if (!isCaretAtEnd(composer)) return;

    const draft = getComposerText(composer);
    const threadMessages = scrapeThreadContext(composer);
    const payload = buildPromptPayload(threadMessages, draft, isRetry);

    cancelGeneration();
    clearSuggestion();

    state.requestCounter += 1;
    const requestId = `req-${state.requestCounter}`;
    state.activeRequestId = requestId;
    state.suggestionDraft = draft;
    state.suggestionText = '';
    state.isStreaming = true;
    state.status = 'generating';
    state.statusDetail = 'Generating suggestion...';
    renderStatus();

    postToWorker({
      type: 'GENERATE',
      requestId,
      modelId: state.modelId,
      prefaceMessages: payload.prefaceMessages,
      userPrompt: payload.userPrompt,
    });
  }

  function scheduleHesitation(composer) {
    clearTimeout(state.debounceTimer);
    if (!state.enabled || !state.cached) return;
    state.debounceTimer = setTimeout(() => {
      if (document.activeElement === composer || composer.contains(document.activeElement)) {
        triggerGeneration(composer, false);
      }
    }, HESITATION_MS);
  }

  // --- Composer Detection & Event Listeners ---
  const boundComposers = new WeakSet();

  function attachComposer(composer) {
    if (boundComposers.has(composer)) return;
    boundComposers.add(composer);

    composer.addEventListener('focus', () => {
      state.activeComposer = composer;
      scheduleHesitation(composer);
    });

    composer.addEventListener('input', () => {
      state.activeComposer = composer;
      if (state.isStreaming || state.suggestionText) {
        cancelGeneration();
        clearSuggestion();
      }
      scheduleHesitation(composer);
    });

    composer.addEventListener('blur', () => {
      clearTimeout(state.debounceTimer);
    });
  }

  function scanComposers(root = document) {
    const nodes = root.querySelectorAll('div[contenteditable="true"]');
    nodes.forEach(attachComposer);
    if (
      document.activeElement &&
      document.activeElement.matches &&
      document.activeElement.matches('div[contenteditable="true"]')
    ) {
      state.activeComposer = document.activeElement;
      attachComposer(document.activeElement);
    }
  }

  document.addEventListener(
    'keydown',
    (e) => {
      if (!state.suggestionText) return;
      if (e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        e.stopPropagation();
        acceptSuggestion();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        cancelGeneration();
        clearSuggestion();
      }
    },
    { capture: true }
  );

  window.addEventListener('resize', positionGhostOverlay);
  window.addEventListener('scroll', positionGhostOverlay, { passive: true, capture: true });

  const observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      for (const node of m.addedNodes) {
        if (node.nodeType !== Node.ELEMENT_NODE) continue;
        if (node.matches?.('div[contenteditable="true"]')) {
          attachComposer(node);
        } else {
          scanComposers(node);
        }
      }
    }
    if (state.activeComposer && !document.contains(state.activeComposer)) {
      state.activeComposer = null;
      cancelGeneration();
      clearSuggestion();
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
  scanComposers();

  // --- Iframe / Worker Message Bridge ---
  window.addEventListener('message', (event) => {
    if (event.origin !== IFRAME_ORIGIN) return;
    const msg = event.data;
    if (!msg || msg.source !== 'autoreply-iframe') return;

    switch (msg.type) {
      case 'IFRAME_READY':
      case 'WORKER_BOOTED': {
        postToWorker({ type: 'CHECK_CACHE', modelId: state.modelId });
        break;
      }

      case 'CACHE_STATUS': {
        state.cached = Boolean(msg.cached);
        if (state.cached) {
          showConsentCard(false);
          state.status = 'loading';
          state.statusDetail = `Cached (${formatBytes(msg.size)}). Initializing WebGPU...`;
          renderStatus();
          postToWorker({ type: 'INIT_ENGINE', modelId: state.modelId });
        } else {
          state.status = 'consent';
          state.statusDetail = 'Model weights not yet cached.';
          renderStatus();
          showConsentCard(true);
        }
        break;
      }

      case 'DOWNLOAD_PROGRESS': {
        state.status = 'downloading';
        state.downloadPercent = msg.percent || 0;
        state.downloadLoaded = msg.loaded || 0;
        state.downloadTotal = msg.total || 0;
        elProgressFill.style.width = `${state.downloadPercent}%`;
        elProgressLabel.textContent = `${state.downloadPercent}% (${formatBytes(msg.loaded)} / ${formatBytes(msg.total)})`;
        state.statusDetail = `Downloading ${state.downloadPercent}%`;
        renderStatus();
        break;
      }

      case 'DOWNLOAD_COMPLETE': {
        state.cached = true;
        showConsentCard(false);
        state.status = 'loading';
        state.statusDetail = 'Download complete. Loading WebGPU...';
        renderStatus();
        postToWorker({ type: 'INIT_ENGINE', modelId: state.modelId });
        break;
      }

      case 'DOWNLOAD_CANCELLED': {
        state.status = 'consent';
        state.statusDetail = 'Download cancelled.';
        elDownloadUi.style.display = 'none';
        renderStatus();
        break;
      }

      case 'CACHE_SKIPPED_QUOTA': {
        state.status = 'loading';
        state.statusDetail = msg.message || 'Streaming directly into WebGPU...';
        renderStatus();
        break;
      }

      case 'ENGINE_LOADING': {
        showConsentCard(false);
        state.status = 'loading';
        state.statusDetail = 'Compiling WebGPU shaders...';
        renderStatus();
        break;
      }

      case 'ENGINE_READY': {
        showConsentCard(false);
        state.cached = true;
        state.status = 'ready';
        state.statusDetail = `Ready (${state.modelId})`;
        renderStatus();
        if (state.activeComposer) {
          scheduleHesitation(state.activeComposer);
        }
        break;
      }

      case 'GENERATION_CHUNK': {
        if (msg.requestId !== state.activeRequestId) return;
        state.suggestionText += msg.chunk || '';
        positionGhostOverlay();
        break;
      }

      case 'GENERATION_DONE': {
        if (msg.requestId !== state.activeRequestId) return;
        state.isStreaming = false;
        state.activeRequestId = null;
        state.status = 'ready';
        state.statusDetail = `Ready (${state.modelId})`;
        renderStatus();
        positionGhostOverlay();
        break;
      }

      case 'GENERATION_CANCELLED': {
        state.isStreaming = false;
        if (state.status === 'generating') {
          state.status = 'ready';
          renderStatus();
        }
        break;
      }

      case 'WORKER_ERROR': {
        showConsentCard(false);
        elDownloadUi.style.display = 'none';
        state.isStreaming = false;
        state.activeRequestId = null;
        state.status = 'error';
        state.statusDetail = msg.error || 'Worker / WebGPU error';
        renderStatus();
        state.drawerOpen = true;
        elDrawer.classList.add('open');
        break;
      }

      default:
        break;
    }
  });

  window.__autoreply = {
    state,
    reactivate() {
      state.drawerOpen = !state.drawerOpen;
      elDrawer.classList.toggle('open', state.drawerOpen);
      if (!state.cached && state.status === 'consent') {
        showConsentCard(true);
      }
      if (state.status === 'error') {
        postToWorker({ type: 'RESTART_WORKER' });
      }
    },
  };

  renderStatus();
})();
