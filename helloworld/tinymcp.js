// @ts-check

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

  /**
   * First call to MCP.
   * @param {{ protocolVersion?: string, capabilities?: any, clientInfo?: any }} [_]
   */
  initialize({ protocolVersion, capabilities, clientInfo } = {}) {
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
      tools: getInfo(this.tools).map(([name, info]) => info)
    };
  }

  async 'tools/call'({ name, arguments: args }) {
    if (!this.tools[name])
      throw new McpError(`Tool '${name}' not found`, -32601, `The tool '${name}' is not recognized by this server.`);

    const structuredContent = this.tools[name](args);

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(structuredContent, null, 2),
        }
      ],
      structuredContent
    };
  }

}

class Tools {
  random() {
    return { result: Math.random() };
  }

  'random:tool' = {
    name: 'random',
    description: 'Generates a random number between 0 (inclusive) and 1 (exclusive).',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    },
    outputSchema: {
      type: 'object', // cannot be anything else for Gemini CLI
      properties: {
        result: {
          type: 'number',
          description: 'A floating-point number between 0 and 1.'
        }
      }
    },
  };
}

function runMcp() {
  const readline = require('readline');

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

async function localInstall() {
  const fs = require('fs');
  const path = require('path');
  const os = require('os');

  const { name, version } = require('./package.json');

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
  const { name, version } = require('./package.json');

  process.stdout.write(name + ' v' + version);
  const [_node, _script, cmd] = process.argv;
  if (cmd === 'install') {
    console.log();
    return localInstall();
  }

  const mcp = new McpServer();
  if (mcp[cmd]) {
    process.stdout.write('\n  MCP ' + JSON.stringify(cmd) + '...');
    const result = await mcp[cmd](params());
    console.log(' ', result);
  } else if (mcp.tools[cmd]) {
    process.stdout.write('\n  MCP command ' + JSON.stringify(cmd) + '...');
    const result = mcp.tools[cmd](params());
    console.log(' ', result);
  } else {
    console.log(
      '\n' +
      (cmd ? 'Unknown command ' + cmd + '.\n' : '') +
      '\nAvailable commands:\n' +
      '  install - Installs the MCP server locally.\n' +
      getInfo(mcp).map(([key]) => '  ' + key + ' - MCP method').join('\n') + '\n' +
      getInfo(mcp.tools).map(([key, info]) => '  ' + key + (info ? ' - ' + info.description : ' - MCP tool')).join('\n')
    );
  }

  function params() {
    if (process.argv.length < 4) return undefined;

    try { return JSON.parse(process.argv.slice(3).join(' ')); }
    catch (e) { return eval('(' + process.argv.slice(3).join(' ') + ')'); }
  }
}

function getInfo(obj) {
  return Object.getOwnPropertyNames(Object.getPrototypeOf(obj))
    .filter(name => typeof obj[name] === 'function' && name !== 'constructor')
    .map(name => [name, obj[name + ':tool']]);
}

if (require.main === module) {
  if (process.stdin.isTTY && process.stdout.isTTY) {
    runInteractive();
  } else {
    runMcp();
  }
}