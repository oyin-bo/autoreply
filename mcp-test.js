const { exec } = require('child_process');
const fs = require('fs');
const path = require('path');

// The test cases are defined here with embedded regex patterns.
// Use ${/pattern/flags} syntax for regex assertions.
const testContent = `
# This is a comment. Lines starting with # are ignored.

# Successful Profile Lookup
# Checks for a successful tool call with keyword confirmation.
> Please fetch BlueSky profile for autoreply.ooo.
< ${/WFH/i}

# Handling Invalid Input (Error Recovery)
# Ensures the server returns an error for a non-existent account.
> Try to use the autoreply profile tool to fetch information about nonexistent-user-99999.bsky.social. Tell me if it worked or failed.
< ${/failed|error/i}
< ${/did_resolve|DID resolution|status 400/i}

# Authentication Elicitation Test
# Tests that the login tool correctly elicits missing credentials.
> Call the autoreply login tool with empty credentials. What does it ask for?
< ${/handle|bluesky/i}

# Search Tool Verification
# Verifies the search tool exists and can be described.
> Does the autoreply server have a 'search' tool? If yes, what does it do?
< ${/yes/i}
< ${/search|posts/i}
`;

// --- Test Harness Implementation ---

const COLORS = {
    red: '\x1b[31m',
    green: '\x1b[32m',
    yellow: '\x1b[33m',
    cyan: '\x1b[36m',
    reset: '\x1b[0m'
};

function parseTestCases(content) {
    const lines = content.split('\n');
    const testCases = [];
    let currentCase = null;

    for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.startsWith('#') || !trimmed) {
            continue;
        }

        if (trimmed.startsWith('>')) {
            if (currentCase) {
                testCases.push(currentCase);
            }
            currentCase = {
                prompt: trimmed.substring(1).trim(),
                assertions: [],
                name: `Test case starting with prompt: "${trimmed.substring(1, 50)}..."`
            };
        } else if (trimmed.startsWith('<') && currentCase) {
            // Extract regex pattern from ${/pattern/flags} syntax
            const assertionText = trimmed.substring(1).trim();
            const regexMatch = assertionText.match(/^\$\{\/(.+)\/([gimsuvy]*)\}$/);
            
            if (regexMatch) {
                const pattern = regexMatch[1];
                const flags = regexMatch[2] || '';
                currentCase.assertions.push({
                    regex: new RegExp(pattern, flags),
                    display: `/${pattern}/${flags}`
                });
            } else {
                // Fallback for non-regex assertions (literal string match)
                currentCase.assertions.push({
                    regex: new RegExp(assertionText.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i'),
                    display: assertionText
                });
            }
        }
    }
    if (currentCase) {
        testCases.push(currentCase);
    }
    return testCases;
}

function runGeminiPrompt(prompt) {
    // Replace placeholders like {{ENV_VAR}}
    const finalPrompt = prompt.replace(/\{\{(\w+)\}\}/g, (match, varName) => {
        return process.env[varName] || '';
    });

    return new Promise((resolve, reject) => {
        // DO NOT use --output-format json, we want plain text responses
    // Always use Flash model to avoid pro quota issues
    const command = `gemini -p "${finalPrompt.replace(/"/g, '\\"')}" -m gemini-2.5-flash`;
        exec(command, { timeout: 60000 }, (error, stdout, stderr) => {
            const combined = `${stdout || ''}\n${stderr || ''}`;

            // Detect MCP ERROR JSON blocks and fail fast with parsed content
            const mcpErrorMatch = combined.match(/MCP ERROR \([^\)]+\):\s*(\[[\s\S]*?\])\s*/i);
            if (mcpErrorMatch) {
                const jsonText = mcpErrorMatch[1];
                try {
                    const parsed = JSON.parse(jsonText);
                    const pretty = JSON.stringify(parsed, null, 2);
                    return reject(new Error(`MCP ERROR detected from server:\n${pretty}`));
                } catch (e) {
                    return reject(new Error(`MCP ERROR detected but JSON parse failed: ${e.message}\nRaw: ${jsonText}`));
                }
            }

            // Any non-zero exit from Gemini CLI is fatal now; surface the output
            if (error) {
                return reject(new Error(`Gemini CLI exited with error: ${error.message}\nOutput:\n${combined}`));
            }

            resolve(combined);
        });
    });
}

async function main() {
    const serverDir = path.basename(process.cwd());
    console.log(`${COLORS.cyan}--- Running E2E tests for: ${serverDir} ---${COLORS.reset}\n`);

    const testCases = parseTestCases(testContent);
    let passed = 0;
    let failed = 0;

    const singleIndex = process.env.SINGLE_TEST_INDEX ? parseInt(process.env.SINGLE_TEST_INDEX, 10) - 1 : -1;
    for (let i = 0; i < testCases.length; i++) {
        if (singleIndex >= 0 && i !== singleIndex) continue;
        const testCase = testCases[i];
        console.log(`${COLORS.yellow}Running test ${i + 1}/${testCases.length}: ${testCase.name}${COLORS.reset}`);

        try {
            const output = await runGeminiPrompt(testCase.prompt);
          const normalized = output;
            let allAssertionsPassed = true;

            for (const assertion of testCase.assertions) {
                const ok = assertion.regex.test(normalized)
                    || assertion.regex.test(output)
                    || assertion.regex.test(normalized.toLowerCase())
                    || assertion.regex.test(output.toLowerCase());
                if (!ok) {
                    allAssertionsPassed = false;
                    console.error(`${COLORS.red}  ✗ FAILED assertion: ${assertion.display}${COLORS.reset}`);
                    // Diagnostic output to aid debugging
                    try {
                        console.error('    regex:', assertion.regex.toString());
                    } catch (e) {
                        console.error('    (could not stringify regex)');
                    }
                    console.error('    normalized snippet:', JSON.stringify(normalized.substring(0, 300)));
                    console.error('    raw snippet:', JSON.stringify(output.substring(0, 300)));
                }
            }

            if (allAssertionsPassed) {
                console.log(`${COLORS.green}  ✓ PASSED${COLORS.reset}\n`);
                passed++;
            } else {
                console.error(`${COLORS.red}  Test failed. Output (first 500 chars):${COLORS.reset}\n${output.substring(0, 500)}\n`);
                failed++;
            }
        } catch (e) {
            console.error(`${COLORS.red}  ✗ FAILED with error:${COLORS.reset}\n${e}\n`);
            failed++;
        }
    }

    console.log('--- Test Summary ---');
    console.log(`Total: ${testCases.length}, Passed: ${passed}, Failed: ${failed}`);
    console.log('--------------------\n');

    if (failed > 0) {
        process.exit(1);
    }
}

main();
