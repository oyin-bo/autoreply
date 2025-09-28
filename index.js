#!/usr/bin/env node
// @ts-check

const fs = require('fs');
const path = require('path');
const os = require('os');
const readline = require('readline');
const readlineSync = require('readline-sync');

const { name, version } = require('./package.json');

(async () => {
  const { Tools } = await import('./src/tools.js');
  // Log proxy status for debugging
  const nodeVersion = parseInt(process.version.slice(1).split('.')[0]);
  const hasProxyVars = !!(process.env.HTTP_PROXY || process.env.http_proxy ||
    process.env.HTTPS_PROXY || process.env.https_proxy ||
    process.env.ALL_PROXY || process.env.all_proxy);

  if (hasProxyVars && nodeVersion < 24) {
    console.error(`[PROXY] Detected proxy environment variables, using custom proxy-aware fetch (Node.js ${process.version})`);
  } else if (hasProxyVars && nodeVersion >= 24) {
    console.error(`[PROXY] Detected proxy environment variables, using native fetch with proxy support (Node.js ${process.version})`);
  }


  // @ts-ignore Adding custom property to prototype
  SyntaxError.prototype.mcpCode = -32700;

  class McpError extends Error {
    /**
     * @param {string} message The error message.
     * @param {number} code The JSON-RPC error code.
     * @param {any} [extra] Extra error data (maps to mcpExtra).
     */
    constructor(message, code, extra = null) {
      super(message);
      this.name = 'McpError';
      /** @type {number} */
      this.mcpCode = code;
      /** @type {any} */
      this.mcpExtra = extra;
    }
  }

  class McpServer {

    tools = new Tools();
    passJsonAsText = false;

    /**
     * First call to MCP.
     * @param {{ protocolVersion?: string, capabilities?: any, clientInfo?: any }} [_]
     */
    initialize({ protocolVersion, capabilities, clientInfo } = {}) {
      if (typeof clientInfo?.name === 'string' && clientInfo.name.toLowerCase().includes('gemini'))
        this.passJsonAsText = true;

      return {
        protocolVersion: '2025-06-18',
        capabilities: {
          tools: {
            listChanged: false, // We don't support dynamic tool list changes for this skeleton
          },
          // resources: {}, // No resource capabilities for this skeleton
          // prompts: {},   // No prompt capabilities for this skeleton
        },
        serverInfo: {
          name: 'random-number-mcp-server',
          version: '1.0.0',
        },
      };
    }

    /**
     * Implementation is required, but does not need to do anything.
     */
    'notifications/initialized'() {
    }

    /** Does not get called at least in Gemini CLI */
    shutdown() {
      process.nextTick(() => process.exit(0));
    }

    'tools/list'() {
      return {
        tools: getInfo(this.tools).map(([name, info]) => info).filter(Boolean)
      };
    }

    async 'tools/call'({ name, arguments: args }) {
      if (!/** @type {*} */(this.tools)[name])
        throw new McpError(`Tool '${name}' not found`, -32601, `The tool '${name}' is not recognized by this server.`);

      const structuredContent = await /** @type {*} */(this).tools[name](args);
      let text = structuredContent?.text;
      if (text || typeof text === 'string')
        delete structuredContent.text;

      console.error('Tool ' + name + ': ', args, text);
      if (this.passJsonAsText && structuredContent) {
        text = JSON.stringify(structuredContent);
      }

      return {
        content: [
          {
            type: 'text',
            text
          }
        ],
        structuredContent
      };
    }
  }

  function runMcp() {

    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout, // Not strictly needed for output, but good practice for readline
      terminal: false // Set to false when reading from non-interactive streams like pipes
    });

    const mcp = new McpServer();

    // Listen for each line of input
    rl.on('line', async (line) => {
      let request = undefined;
      if (!line) return;
      try {
        request = JSON.parse(line);

        // Log to stderr for debugging, as stdout is for protocol messages
        // console.error('Request ', request);

        if (!mcp[request.method])
          throw new McpError(`Method '${request.method}' not found`, -32601, `The method '${request.method}' is not recognized by this server.`);

        const result = await mcp[request.method](request.params);

        // If requestID is undefined/null (notification), no response is sent.
        if (typeof request.id !== 'undefined') { // Check if it's a request (has an ID)
          // console.error('Response ', result);
          process.stdout.write(
            JSON.stringify({
              jsonrpc: '2.0',
              id: request.id,
              result
            }) + '\n');
        }

      } catch (e) {
        // console.error(`Error processing line (request ID ${request?.id || 'N/A'}):`, e);

        process.stdout.write(
          JSON.stringify({
            jsonrpc: '2.0',
            id: request?.id,
            error: {
              code: e.mcpCode || -32000,
              message: e.message,
              data: e.mcpExtra || e.stack
            }
          }) + '\n');
      }
    });

    rl.on('close', () => {
      process.exit(0);
    });

    rl.on('error', (err) => {
      // console.error('Readline error:', err);
      process.stdout.write(
        JSON.stringify({
          jsonrpc: '2.0',
          id: null,
          error: {
            code: -32000, // Internal error
            message: err.message || 'Internal error',
            data: 'Readline stream error occurred.',
          },
        }) + '\n');
      process.exit(1);
    });

  }

  function getInfo(obj) {
    return Object.getOwnPropertyNames(Object.getPrototypeOf(obj))
      .filter(name => typeof obj[name] === 'function' && name !== 'constructor')
      .map(name => [name, obj[name + ':tool']]);
  }


  async function localLogin() {
    try {
      const mcp = new McpServer();

      process.stdout.write('BlueSky login for ' + name + ' v' + version + ' MCP');
      const keytar = await keytarOrPromise;
      console.log();
      const login = readlineSync.question('   account: ');
      const password = readlineSync.question('  password: ', { hideEchoBack: true, mask: '*' });
      process.stdout.write('    access..');
      const feed = await mcp.tools.feed({ login, password });
      process.stdout.write('.');
      const profile = await mcp.tools.profile({ user: login });
      process.stdout.write('\n\nLogged in as @' + profile.handle + ' ' + profile.displayName);
      await keytar.setPassword(name, login, password);
      await keytar.setPassword(name, 'default_handle', login);
      console.log();
      if (feed.posts.length) {
        for (let i = 0; i < feed.posts.length && i < 4; i++) {
          const post = feed.posts[i];
          console.log('  ' + post.indexedAt + ' @' + post.author + ' ' + post.text.trim().split('\n')[0]);
        }
      }
      console.log('\nCredentials stored.');
    } catch (e) {
      console.error('Login failed:', e.message);
    }
  }

  /**
   * Generate a formatted text preview of posts (used for feed and search results)
   * @param {any[]} posts Array of post objects
   * @param {string} [title] Optional title for the preview
   * @param {number} [maxPosts] Maximum number of posts to show (default 20)
   * @returns {string} Formatted text preview
   */
  function generateFeedPreviewText(posts, title = 'Feed', maxPosts = 20) {
    if (!posts || !posts.length) {
      return `${title}: No posts found.`;
    }

    const now = new Date();
    let output = [];
    const sortedPosts = [...posts].sort((a, b) => new Date(b.indexedAt).getTime() - new Date(a.indexedAt).getTime());

    for (const post of sortedPosts) {
      const dtPost = new Date(post.indexedAt);
      const dtStr =
        dtPost.toISOString().split('T')[0] === now.toISOString().split('T')[0] ?
          dtPost.toLocaleTimeString() :
          dtPost.toLocaleDateString();

      const text = post.text.trim().split('\n').filter(ln => ln.trim())[0];
      if (!text) continue;

      output.push(
        '  ' + dtStr.padStart(10) + ' ' + ('@' + post.author).padStart(31, output.length % 2 ? ' ' : '\u00B7 ') + '  ' + (text.length > 60 ? text.slice(0, 65) + '...' : text)
      );

      if (output.length >= maxPosts) break;
    }

    return `${title}:\n` + (output.length ? output.join('\n') : 'No posts found.');
  }

  async function printFeedPreview(params) {
    console.log();

    const mcp = new McpServer();
    const feed = await mcp.tools.feed({ limit: 100, ...params });
    const previewText = generateFeedPreviewText(feed.posts, 'Current feed');
    console.log(previewText);
  }

  async function localInstall() {
    const settingsPath = path.join(os.homedir(), '.gemini', 'settings.json');
    console.log('>Installing ' + name + ' v' + version + ' MCP server');
    process.stdout.write('  for Gemini CLI ' + settingsPath);
    fs.mkdirSync(path.dirname(settingsPath), { recursive: true });
    process.stdout.write('.');
    let settingsJson = {};
    if (fs.existsSync(settingsPath)) {
      try { settingsJson = JSON.parse(fs.readFileSync(settingsPath, 'utf8')); } catch { }
    }
    process.stdout.write('.');

    settingsJson = {
      ...settingsJson,
      allowMCPServers: [
        'autoreply',
        ...(settingsJson.allowMCPServers || []).filter(server => server !== 'autoreply'),
      ],
      mcpServers: {
        ...settingsJson.mcpServers,
        autoreply: {
          ...settingsJson.mcpServers?.autoreply,
          type: 'stdio',
          command: 'node',
          args: [
            path.resolve(__filename)
          ],
        }
      }
    };

    process.stdout.write('.');
    fs.writeFileSync(settingsPath, JSON.stringify(settingsJson, null, 2));
    console.log(' OK');

    const mcpJsonPath =
      process.platform === 'win32' ?
        path.join(process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming'), 'Code', 'User', 'mcp.json') :
        process.platform === 'darwin' ?
          path.join(os.homedir(), 'Library', 'Application Support', 'Code', 'User', 'mcp.json') :
          !!process.env.CODESPACES || !!process.env.CODESPACE_NAME ?
            path.join(os.homedir(), '.vscode-remote', 'data', 'User', 'mcp.json') :
            path.join(os.homedir(), '.config', 'Code', 'User', 'mcp.json');
    process.stdout.write('  for VS Code    ' + mcpJsonPath + '..');
    fs.mkdirSync(path.dirname(mcpJsonPath), { recursive: true });
    process.stdout.write('.');

    let mcpJson = {};
    if (fs.existsSync(mcpJsonPath)) {
      try { mcpJson = JSON.parse(fs.readFileSync(mcpJsonPath, 'utf8')); } catch { }
    }

    mcpJson = {
      ...mcpJson,
      servers: {
        ...mcpJson.servers,
        autoreply: {
          ...mcpJson.servers?.autoreply,
          name: 'autoreply',
          type: 'stdio',
          command: 'node',
          args: [
            path.resolve(__filename)
          ]
        }
      }
    };

    fs.writeFileSync(mcpJsonPath, JSON.stringify(mcpJson, null, 2));
    console.log(' OK.');

    console.log('Successfully installed for ' + path.resolve(__filename));
  }

  async function runInteractive() {
    process.stdout.write(name + ' v' + version);
    const [_node, _script, cmd] = process.argv;
    if (cmd === 'install') {
      console.log();
      return localInstall();
    }

    const mcp = new McpServer();
    if (mcp[cmd]) {
      process.stdout.write('\n  MCP ' + JSON.stringify(cmd) + '...');
      const result = await mcp[cmd](params(cmd) || {});
      console.log(' ', result);
    } else if (mcp.tools[cmd]) {
      process.stdout.write('\n  MCP command ' + JSON.stringify(cmd) + '...');
      const result = await mcp.tools[cmd](params(cmd) || {});
      console.log(' ', result);
    } else {
      console.log(
        '\n' +
        (cmd ? 'Unknown command ' + cmd + '.\n' : '') +
        '\nAvailable commands:\n' +
        '  install - Installs the MCP server locally.\n' +
        getInfo(mcp).map(([key]) => '  ' + key + ' - MCP method').join('\n') + '\n' +
        getInfo(mcp.tools).map(([key, info]) => '  ' + key + (info ? ' - ' + info.description : ' - extra')).join('\n')
      );
      printFeedPreview(params(cmd));
    }

    function params(cmd) {
      if (process.argv.length < 4) return undefined;

      const raw = process.argv.slice(3).join(' ');

      // Try JSON first (e.g. '{"text":"hi"}' or '"string"')
      try {
        return JSON.parse(raw);
      } catch (e) {
        // ignore and try eval next
      }

      // Try eval for JS literals like ({a:1}) or [1,2]
      try {
        // eslint-disable-next-line no-eval
        return eval('(' + raw + ')');
      } catch (e) {
        // If eval fails (e.g., bare word like Microsoft), fall through to heuristics
      }

      // Heuristic fallback: if the tool has an input schema, map the bare string to a likely key
      try {
        const toolInfo = cmd ? mcp.tools[cmd + ':tool'] : undefined;
        const props = toolInfo?.inputSchema?.properties || {};
        if ('query' in props) return { query: raw };
        if ('text' in props) return { text: raw };
        if ('user' in props) return { user: raw };
        if ('postURI' in props) return { postURI: raw };
        if ('login' in props && 'password' in props) return { login: raw };
      } catch (e) {
        // ignore heuristics errors
      }

      // Default fallback: return an object with `query` so common commands like `search` work
      return { query: raw };
    }
  }


  if (require.main === module) {
    if (process.stdin.isTTY && process.stdout.isTTY) {
      runInteractive();
    } else {
      runMcp();
    }
  }

})();