// @ts-check

import fs from 'fs';
import os from 'os';
import path from 'path';
import readlineSync from 'readline-sync';

import package_json from '../package.json' with { type: 'json' };

import { getInfo, McpServer, runMcpStdio } from './mcp.js';
import keytarOrPromise from './keytar.js';

if (process.stdin.isTTY && process.stdout.isTTY) {
  runInteractive();
} else {
  runMcpStdio();
}

async function runInteractive() {
  process.stdout.write(package_json.name + ' v' + package_json.version);
  const [_node, _script, cmd] = process.argv;
  if (cmd === 'install') {
    console.log();
    return localInstall();
  } else if (cmd === 'login') {
    console.log();
    return localLogin();
  }

  /** @type {*} */
  const mcp = new McpServer();
  if (mcp[cmd]) {
    process.stdout.write('\n  MCP ' + JSON.stringify(cmd) + '...');
    const result = await mcp[cmd](parseCmdParams(cmd) || {});
    console.log(' ', result);
  } else if (mcp.tools[cmd]) {
    process.stdout.write('\n  MCP command ' + JSON.stringify(cmd) + '...');
    const result = await mcp.tools[cmd](parseCmdParams(cmd) || {});
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
    printFeedPreview(parseCmdParams(cmd));
  }

  /**
   * @param {string} cmd
   */
  function parseCmdParams(cmd) {
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

async function localInstall() {
  const settingsPath = path.join(os.homedir(), '.gemini', 'settings.json');
  console.log('>Installing MCP server');
  process.stdout.write('  for Gemini CLI ' + settingsPath);
  fs.mkdirSync(path.dirname(settingsPath), { recursive: true });
  process.stdout.write('.');
  /** @type {*} */
  let settingsJson = {};
  if (fs.existsSync(settingsPath)) {
    try { settingsJson = JSON.parse(fs.readFileSync(settingsPath, 'utf8')); } catch { }
  }
  process.stdout.write('.');

  settingsJson = {
    ...settingsJson,
    allowMCPServers: [
      'autoreply',
      ...(settingsJson.allowMCPServers || []).filter(
        /** @param {string} server */(server) => server !== 'autoreply'),
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

  /** @type {*} */
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

async function localLogin() {
  try {
    const mcp = new McpServer();

    process.stdout.write('BlueSky login');
    const keytar = await keytarOrPromise;
    console.log();
    const login = readlineSync.question('   account: ');
    const password = readlineSync.question('  password: ', { hideEchoBack: true, mask: '*' });
    process.stdout.write('    access..');
    const feed = await mcp.tools.feed({ login, password });
    process.stdout.write('.');
    const profile = await mcp.tools.profile({ user: login });
    process.stdout.write('\n\nLogged in as @' + profile.handle + ' ' + profile.displayName);
    await keytar.setPassword(package_json.name, login, password);
    await keytar.setPassword(package_json.name, 'default_handle', login);
    console.log();
    if (feed.posts.length) {
      for (let i = 0; i < feed.posts.length && i < 4; i++) {
        const post = feed.posts[i];
        console.log('  ' + post.indexedAt + ' @' + post.author + ' ' + post.text.trim().split('\n')[0]);
      }
    }
    console.log('\nCredentials stored.');
  } catch (e) {
    console.error('Login failed:', /** @type {*} */(e).message);
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

    const text = /** @type {string} */(post.text).trim().split('\n').filter(ln => ln.trim())[0];
    if (!text) continue;

    output.push(
      '  ' + dtStr.padStart(10) + ' ' + ('@' + post.author).padStart(31, output.length % 2 ? ' ' : '\u00B7 ') + '  ' + (text.length > 60 ? text.slice(0, 65) + '...' : text)
    );

    if (output.length >= maxPosts) break;
  }

  return `${title}:\n` + (output.length ? output.join('\n') : 'No posts found.');
}

/** @param {*} feedParams */
async function printFeedPreview(feedParams) {
  console.log();

  const mcp = new McpServer();
  const feed = await mcp.tools.feed({ limit: 100, ...feedParams });
  const previewText = generateFeedPreviewText(feed.posts, 'Current feed');
  console.log(previewText);
}