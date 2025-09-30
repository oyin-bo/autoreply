# Trial Mode: Command-Line Utility Functionality

## 1. Overview

The autoreply servers (both Rust and Go implementations) currently operate exclusively as Model Context Protocol (MCP) servers using stdio communication. This document specifies the requirements for adding a "trial mode" that allows the same binary to function as a traditional command-line utility for testing and debugging purposes.

## 2. Functional Requirements

### 2.1 Dual-Mode Operation

The `autoreply` binary MUST support two operational modes:

1. **MCP Server Mode** (default): When invoked without command-line arguments
   - Behavior: Standard stdio-based MCP JSON-RPC 2.0 protocol server
   - Communication: Line-delimited JSON over stdin/stdout
   - Lifecycle: Long-running process until EOF or termination signal

2. **Trial/CLI Mode**: When invoked with command-line arguments
   - Behavior: Execute a single tool/command and exit
   - Communication: Standard command-line arguments and output
   - Lifecycle: Single execution, immediate exit with appropriate status code

### 2.2 Command-Line Interface

When operating in Trial/CLI mode, the interface MUST be:

```bash
autoreply <tool> [OPTIONS]
```

Where:
- `<tool>` is the tool/command name (e.g., `profile`, `search`)
- `[OPTIONS]` are tool-specific arguments in standard CLI format

#### 2.2.1 Argument Formats

Both formats MUST be supported:
- GNU-style with equals: `autoreply profile --account=alice.bsky.social`
- Space-separated: `autoreply profile --account alice.bsky.social`

#### 2.2.2 Available Commands

**Profile Tool:**
```bash
autoreply profile --account <handle-or-did>
autoreply profile -a <handle-or-did>
```

**Search Tool:**
```bash
autoreply search --account <handle-or-did> --query <search-terms> [--limit <number>]
autoreply search -a <handle-or-did> -q <search-terms> [-l <number>]
```

### 2.3 Output

In Trial/CLI mode:
- **Success**: Output the tool result to stdout as markdown
- **Error**: Output error message to stderr and exit with non-zero status code
- **Help**: Support `--help` and `-h` flags for usage information
- **Version**: Support `--version` and `-V` flags

### 2.4 Exit Codes

- `0`: Successful execution
- `1`: Invalid arguments or usage error
- `2`: Network or API error (transient)
- `3`: Not found error (e.g., account doesn't exist)
- `4`: Timeout error
- `5`: Other application errors

## 3. Technical Requirements

### 3.1 Schema Unification

The parameter schemas MUST be defined once and used for both:
1. MCP tool JSON schema (for `tools/list` response)
2. CLI argument parsing and help generation

This ensures consistency between MCP and CLI interfaces.

### 3.2 Code Organization

The implementation SHOULD use:
- **Rust**: Derive macros from `clap` v4+ for schema extraction
- **Go**: Struct tags or code generation for schema unification

### 3.3 Shared Logic

The tool execution logic MUST be shared between MCP and CLI modes. Only the:
- Input parsing layer (JSON-RPC vs CLI args)
- Output formatting layer (JSON-RPC response vs CLI output)

should differ.

### 3.4 Configuration

Both modes SHOULD respect the same environment variables and configuration:
- Cache settings
- TTL configurations
- Proxy settings
- Logging levels

### 3.5 Logging

- **MCP Mode**: Use tracing/logging to stderr as currently implemented
- **CLI Mode**: 
  - Default: Minimal output (only results)
  - `--verbose` or `-v`: Enable detailed logging to stderr
  - `--quiet` or `-q`: Suppress all non-error output

## 4. Design Constraints

### 4.1 No Breaking Changes

The default behavior (no arguments = MCP server mode) MUST be preserved to maintain compatibility with existing MCP clients.

### 4.2 Binary Size

The addition of CLI parsing SHOULD NOT significantly increase binary size (target: <5% increase).

### 4.3 Dependencies

Prefer well-maintained, standard CLI parsing libraries:
- **Rust**: `clap` (with derive feature)
- **Go**: `cobra` + `viper` OR `kong`

### 4.4 Error Messages

CLI error messages MUST be user-friendly and actionable, suggesting:
- Correct usage syntax
- Valid values for parameters
- Common troubleshooting steps

## 5. Non-Functional Requirements

### 5.1 Performance

CLI mode execution overhead compared to direct tool execution SHOULD be <50ms.

### 5.2 Testing

- Unit tests MUST validate CLI argument parsing
- Integration tests MUST verify both MCP and CLI modes produce identical results
- CI/CD MUST include CLI mode smoke tests

### 5.3 Documentation

- README MUST include CLI usage examples
- `--help` output MUST be comprehensive and well-formatted
- Man pages or shell completion scripts are OPTIONAL but recommended

## 6. Implementation Phases

### Phase 1: Core CLI Support
- Basic argument parsing
- Tool execution in CLI mode
- Standard output formatting

### Phase 2: Enhanced Features
- Shell completion generation
- Interactive mode (prompting for missing args)
- Color output and enhanced formatting

### Phase 3: Advanced Features (Optional)
- Config file support for CLI mode
- Batch operations (reading multiple queries from file)
- Watch mode (continuous execution)

## 7. Example Usage

```bash
# Get profile information
$ autoreply profile --account alice.bsky.social
# @alice.bsky.social (did:plc:...)
**Display Name:** Alice Smith
**Description:** Software engineer...

# Search posts
$ autoreply search --account bob.bsky.social --query "rust programming" --limit 10

# Verbose logging
$ autoreply search -a bob.bsky.social -q "rust" -v

# Get help
$ autoreply --help
$ autoreply profile --help

# Run as MCP server (existing behavior)
$ autoreply
```

## 8. Success Criteria

The implementation is considered successful when:

1. ✅ Binary runs as MCP server by default (no arguments)
2. ✅ Binary accepts CLI arguments and executes tools
3. ✅ CLI and MCP modes share the same tool schemas
4. ✅ CLI provides helpful error messages and `--help` output
5. ✅ All existing tests pass
6. ✅ New integration tests verify dual-mode operation
7. ✅ Documentation includes CLI usage examples

## 9. Future Considerations

- **Interactive Shell**: A REPL mode for exploring profiles and posts
- **Piping**: Support for piping output between multiple autoreply commands
- **Parallel Execution**: Batch processing of multiple accounts
- **Color Output**: Rich terminal formatting with syntax highlighting

---

## 10. Implementation Plans Summary

Detailed implementation plans for both Rust and Go are provided in separate documents:

### Rust Implementation (Recommended Libraries)

**Primary Library: `clap` v4 with derive macros**

```toml
[dependencies]
clap = { version = "4.5", features = ["derive", "env", "unicode", "wrap_help"] }
schemars = "0.8"  # For JSON Schema generation from Rust structs
```

**Key Features:**
- **Derive Macros**: Use `#[derive(Parser)]` on structs to auto-generate CLI parsing
- **Schema Generation**: Use `schemars` to generate JSON schemas from the same structs
- **Unified Types**: Define argument structs once, use everywhere
- **Zero Runtime Cost**: All parsing and validation at compile time

**Architecture Pattern:**
```rust
// Define arguments with both clap and schemars derives
#[derive(Parser, JsonSchema, Deserialize)]
struct ProfileArgs {
    #[arg(short = 'a', long)]
    account: String,
}

// Tool execution is mode-agnostic
async fn execute_tool(args: ProfileArgs) -> Result<String> {
    // Shared business logic
}

// Thin adapters for MCP and CLI
async fn mcp_handler(json_args: Value) -> McpResponse { /* ... */ }
fn cli_main() { /* ... */ }
```

**Schema Extraction:** Automatic via `schemars::schema_for!(ProfileArgs)` produces valid JSON Schema compatible with MCP `tools/list`.

---

### Go Implementation (Recommended Libraries)

**Primary Library: `cobra` + struct tags**

```go
import (
    "github.com/spf13/cobra"              // CLI framework
    "github.com/invopop/jsonschema"       // JSON Schema from structs
)
```

**Key Features:**
- **Struct Tags**: Use custom tags for schema metadata
- **Cobra Subcommands**: Professional CLI with git-style subcommands
- **Reflection**: Auto-generate schemas and CLI flags from struct definitions
- **Tool Registry**: Central registry pattern for managing all tools

**Architecture Pattern:**
```go
// Define arguments with rich tags
type ProfileArgs struct {
    Account string `json:"account" jsonschema:"required" description:"Handle or DID" short:"a"`
}

// Tool definition combines metadata and execution
type ToolDefinition struct {
    Name        string
    Description string
    Args        interface{}
    Execute     func(ctx context.Context, args interface{}) (string, error)
}

// Automatic schema extraction
func (td *ToolDefinition) ExtractSchema() mcp.InputSchema {
    return jsonschema.Reflect(td.Args)
}

// Automatic CLI command generation
func createCobraCommand(td ToolDefinition) *cobra.Command {
    // Uses reflection to build flags from struct tags
}
```

**Schema Extraction:** Reflection-based using `invopop/jsonschema` to generate JSON Schema from struct tags, ensuring MCP and CLI stay synchronized.

---

### Implementation Comparison

| Aspect | Rust (clap + schemars) | Go (cobra + jsonschema) |
|--------|------------------------|-------------------------|
| **Ergonomics** | ⭐⭐⭐⭐⭐ Derive macros | ⭐⭐⭐⭐ Struct tags |
| **Type Safety** | ⭐⭐⭐⭐⭐ Compile-time | ⭐⭐⭐⭐ Runtime checks |
| **Boilerplate** | ⭐⭐⭐⭐⭐ Minimal | ⭐⭐⭐ Moderate |
| **Performance** | ⭐⭐⭐⭐⭐ Zero cost | ⭐⭐⭐⭐ Negligible overhead |
| **Ecosystem** | ⭐⭐⭐⭐⭐ clap is standard | ⭐⭐⭐⭐⭐ cobra is standard |

### Shared Implementation Principles

Both implementations follow these core principles:

1. **Single Source of Truth**: Argument definitions are written once
2. **Decorator/Attribute Pattern**: Use language features (derives/tags) for metadata
3. **Schema Unification**: MCP JSON schemas generated from CLI argument definitions
4. **Thin Adapters**: MCP and CLI are just I/O layers over shared business logic
5. **Mode Detection**: Check `args.len()` to determine MCP vs CLI mode

### Example: Adding a New Tool

**Rust:**
```rust
// 1. Define args struct
#[derive(Parser, JsonSchema, Deserialize)]
struct NewToolArgs {
    #[arg(short, long)]
    param: String,
}

// 2. Implement execute function
async fn execute_new_tool(args: NewToolArgs) -> Result<String> { /* ... */ }

// 3. Add to CLI commands enum
enum Commands {
    NewTool(NewToolArgs),
}

// 4. Add to MCP handler
match tool_name {
    "new_tool" => execute_new_tool(parse(args)).await,
}
```

**Go:**
```go
// 1. Define args struct with tags
type NewToolArgs struct {
    Param string `json:"param" jsonschema:"required" description:"Description" short:"p"`
}

// 2. Implement tool
func (t *NewTool) Definition() ToolDefinition {
    return ToolDefinition{
        Name: "new_tool",
        Args: &NewToolArgs{},
        Execute: t.Execute,
    }
}

// 3. Register in registry
registry.AddTool(newTool.Definition())
```

Both approaches automatically provide:
- CLI help generation
- MCP JSON schema
- Argument validation
- Error handling

### Next Steps

For detailed code examples and step-by-step implementation guides, see:
- `docs/10-trial-rust-implementation.md` - Full Rust implementation guide
- `docs/10-trial-go-implementation.md` - Full Go implementation guide