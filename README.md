# An Interactive Guide to the Model Context Protocol

The Model Context Protocol (MCP) is an open-source standard that enables AI agents to securely interact with external tools and data, extending their capabilities beyond their initial training.

## A Client-Server Architecture

At its core, MCP solves a fundamental problem: AI models often lack up-to-the-minute information and the ability to perform real-world actions. By serving as a secure communication layer, MCP allows an AI (the client) to ask a custom application (the server) for data or to perform a task.

This architecture makes your application the "expert" on a specific domain. The AI client queries your application, and your server provides the answer or performs the action, returning the result to the AI.

Communication between the client and server happens via **JSON-RPC over a transport layer**.

For local development, the `StdioServerTransport` is used. This allows the Gemini CLI to launch your server as a separate, hidden process and communicate with it using the standard input and standard output streams (`stdio`).

When you exit a chat session, the Gemini CLI automatically closes this subprocess, preventing it from running indefinitely in the background.

## Building Your First Bridge

This guide walks through creating a functional weather server in JavaScript. It exposes a single `get_weather` tool that an AI agent can use.

### 1. Set Up Project & Dependencies

First, create a project directory and install the necessary MCP SDK from npm.

```shell
# Create the project directory
mkdir mcp-weather-server
cd mcp-weather-server

# Initialize a new npm project
npm init -y

# Install the MCP SDK
npm install @modelcontextprotocol/sdk
```

### 2. Create the Server File

Create a `server.js` file. This code defines the server, registers the `get_weather` tool with its expected parameters (`inputSchema`), and handles the logic for fetching data from an external weather API.

```JavaScript
// @ts-check
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const server = new Server({ name: "weather-server" });

server.setRequestHandler("ListToolsRequest", async (request) => {
  return {
    tools: [{
      name: "get_weather",
      description: "Gets the current temperature and wind speed for a given latitude and longitude.",
      inputSchema: {
        type: "object",
        properties: {
          latitude: { type: "number", description: "The latitude of the location." },
          longitude: { type: "number", description: "The longitude of the location." }
        },
        required: ["latitude", "longitude"]
      }
    }]
  };
});

server.setRequestHandler("CallToolRequest", async (request) => {
  if (request.params.name === "get_weather") {
    const { latitude, longitude } = request.params.arguments;
    try {
      const response = await fetch(`https://api.open-meteo.com/v1/forecast?latitude=${latitude}&longitude=${longitude}&current_weather=true`);
      const data = await response.json();
      if (response.ok) {
        const weather = data.current_weather;
        return { toolResult: `The current temperature is ${weather.temperature}°C with a wind speed of ${weather.windspeed} km/h.` };
      } else {
        return { toolResult: { error: `API error: ${data.reason}` }, isError: true };
      }
    } catch (error) {
      return { toolResult: { error: `Failed to fetch weather data: ${error.message}` }, isError: true };
    }
  }
  return { toolResult: "Tool not found.", isError: true };
});

const transport = new StdioServerTransport();
await server.connect(transport);
console.log("MCP Weather Server is running via Stdio. Ready for a client to connect...");

```

### 3. Run the Server

Execute the script with Node.js. Your server is now running and waiting for a client to connect.

```shell
node server.js
```

### 4. Connect to an AI Agent

To make the server available to the Gemini CLI, add it to the settings file located at `~/.gemini/settings.json`. This creates a persistent connection for all your chat sessions.

```JSON
{
  "mcpServers": {
    "weather-server": {
      "command": "node",
      "args": [
        "/path/to/your/server.js"
      ]
    }
  }
}
```

With this configuration, simply running `gemini chat` will connect your agent to the server. Other methods, like connecting to a remote server via HTTP, also exist for different use cases.

## Navigating the Pitfalls

Building an MCP server can be complex. Here are some common issues and how to solve them.

* **Transport Mismatch:** `StdioServerTransport` is for local development only. For production, use a different transport like `HttpServerTransport`. The wrong transport will fail the connection. Use the correct transport for your environment.
* **Vague Tool Description:** The AI uses the `description` and `inputSchema` to call your tool. A poor description leads to misuse or no use at all. Be specific about what your tool does and what each parameter is for.
* **Errors from External Services:** Always wrap API calls in a `try/catch` block. The `isError: true` flag tells the AI the call failed and provides a useful error message.
* **Asynchronous Code Issues:** Forgetting to `await` a promise may return a result before the API call is done, sending incomplete data. Always use `async/await` so all operations finish before the response is sent.
* **Missing Dependencies:** If the SDK or other packages aren't installed correctly, the server will not start. The fix is to run `npm install` to ensure all dependencies are in place.

## The Road Ahead

The "hello world" example is just the beginning. MCP opens the door to more powerful and integrated AI applications.

* **Multi-Tool Server:** Expose multiple tools from one server. A financial analysis server can provide tools for stock prices, news, and historical data. The AI can then chain these tools for complex requests.
* **Legacy System Integration:** Wrap an MCP server around a legacy system to expose proprietary tools. This gives your team a new AI-powered interface to old code.
* **Dynamic Tool Registration:** Build a server that discovers and registers tools on the fly. This allows your server to adapt to new capabilities without manual code changes.

