// Package cli provides command-line interface support for trial mode
package cli

import (
	"bufio"
	"fmt"
	"os"
	"strings"
	"syscall"

	"golang.org/x/term"
)

// PromptForInput prompts the user for input and returns the trimmed response
func PromptForInput(prompt string) (string, error) {
	fmt.Fprint(os.Stderr, prompt)
	reader := bufio.NewReader(os.Stdin)
	input, err := reader.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("failed to read input: %w", err)
	}
	return strings.TrimSpace(input), nil
}

// PromptForPassword prompts the user for a password without echoing it
func PromptForPassword(prompt string) (string, error) {
	fmt.Fprint(os.Stderr, prompt)
	
	// Read password without echoing
	bytePassword, err := term.ReadPassword(int(syscall.Stdin))
	fmt.Fprintln(os.Stderr) // Print newline after password input
	
	if err != nil {
		return "", fmt.Errorf("failed to read password: %w", err)
	}
	
	return strings.TrimSpace(string(bytePassword)), nil
}
