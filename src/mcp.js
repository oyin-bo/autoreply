// @ts-check

import readline from 'readline';

import { Tools } from './tools.js';

export function runMcpStdio() {

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

        const method = /** @type {keyof typeof mcp} */(request.method);

        if (typeof mcp[method] !== 'function')
          throw new McpError(`Method '${method}' not found`, -32601, `The method '${method}' is not recognized by this server.`);

        const result = await mcp[method](request.params);

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

      } catch (error) {
        // console.error(`Error processing line (request ID ${request?.id || 'N/A'}):`, e);
        const e = /** @type {any} */(error);

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

export class McpServer {

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

  /**
   * @param {{
   *  name: string,
   *  arguments: any
   * }} _
   */
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

/** @param {*} obj */
export function getInfo(obj) {
  return Object.getOwnPropertyNames(Object.getPrototypeOf(obj))
    .filter(name => typeof obj[name] === 'function' && name !== 'constructor')
    .map(name => [name, obj[name + ':tool']]);
}
