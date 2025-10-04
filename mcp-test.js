const { exec } = require('child_process');
const fs = require('fs');
const path = require('path');

// The test cases are defined here.
const testContent = `
# This is a comment. Lines starting with # are ignored.

# Test Case 1: Tool Discovery
# Verifies that the server starts and reports its tools.
> List all available MCP tools
< /"name": "profile"/
< /"name": "search"/
< /"name": "login"/

# Test Case 2: Successful Profile Lookup
# Checks for a successful tool call and specific content in the response.
> Use the autoreply profile tool to get information about the BlueSky account "bsky.app"
< /"totalSuccess":\\s*[1-9]/
< /bsky\\.app/i

# Test Case 3: Handling Invalid Input
# Ensures the server returns a user-friendly error for a non-existent account.
> Use the autoreply profile tool to get information about "nonexistent-user-12345.bsky.social"
< /not found|error|invalid/i
# Also verify a tool call was attempted, even if it failed.
< /"totalCalls":\\s*[1-9]/

# Test Case 4: Authentication Flow Test
# This test requires BSKY_TEST_HANDLE and BSKY_TEST_PASSWORD environment variables.
> Use the autoreply login tool to authenticate with handle "{{BSKY_TEST_HANDLE}}" and password "{{BSKY_TEST_PASSWORD}}"
< /"totalSuccess":\\s*[1-9]/
< /authenticated|logged in/i

# Test Case 5: Performance Baseline
# Measures typical response times.
> Get the profile for "bsky.app" using autoreply
< /"totalDurationMs":\\s*\\d+/
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
                name: `Test case starting with prompt: "${trimmed.substring(1, 40)}..."`
            };
        } else if (trimmed.startsWith('<') && currentCase) {
            currentCase.assertions.push(new RegExp(trimmed.substring(1).trim(), 'i'));
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
        const command = `gemini -p "${finalPrompt.replace(/"/g, '\\"')}" --output-format json`;
        exec(command, { timeout: 45000 }, (error, stdout, stderr) => {
            if (error) {
                // Gemini CLI often returns errors for non-zero exit codes even with valid JSON.
                // We resolve with the output if it's parsable JSON, otherwise reject.
                try {
                    const jsonOutput = JSON.parse(stdout || stderr);
                    return resolve(JSON.stringify(jsonOutput, null, 2));
                } catch (e) {
                    return reject(`Command failed: ${error.message}\nSTDOUT: ${stdout}\nSTDERR: ${stderr}`);
                }
            }
            resolve(stdout);
        });
    });
}

async function main() {
    const serverDir = path.basename(process.cwd());
    console.log(`${COLORS.cyan}--- Running E2E tests for: ${serverDir} ---${COLORS.reset}\n`);

    const testCases = parseTestCases(testContent);
    let passed = 0;
    let failed = 0;

    for (let i = 0; i < testCases.length; i++) {
        const testCase = testCases[i];
        console.log(`${COLORS.yellow}Running test ${i + 1}/${testCases.length}: ${testCase.name}${COLORS.reset}`);

        try {
            const output = await runGeminiPrompt(testCase.prompt);
            let allAssertionsPassed = true;

            for (const assertion of testCase.assertions) {
                if (!assertion.test(output)) {
                    allAssertionsPassed = false;
                    console.error(`${COLORS.red}  ✗ FAILED assertion: ${assertion}${COLORS.reset}`);
                }
            }

            if (allAssertionsPassed) {
                console.log(`${COLORS.green}  ✓ PASSED${COLORS.reset}\n`);
                passed++;
            } else {
                console.error(`${COLORS.red}  Test failed. Full output:${COLORS.reset}\n${output}\n`);
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
