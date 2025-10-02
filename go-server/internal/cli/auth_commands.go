package cli

import (
	"context"
	"fmt"
	"syscall"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/spf13/cobra"
	"golang.org/x/term"
)

// CreateAuthCommands creates authentication-related CLI commands
func CreateAuthCommands() []*cobra.Command {
	return []*cobra.Command{
		createLoginCommand(),
		createAccountsCommand(),
		createLogoutCommand(),
		createUseCommand(),
	}
}

// createLoginCommand creates the login command
func createLoginCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "login",
		Short: "Authenticate with BlueSky",
		Long:  `Authenticate with BlueSky and store credentials securely in the system keyring.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			handle, _ := cmd.Flags().GetString("handle")
			method, _ := cmd.Flags().GetString("method")
			
			// If handle not provided, prompt for it
			if handle == "" {
				fmt.Print("BlueSky handle: ")
				fmt.Scanln(&handle)
			}
			
			ctx := context.Background()
			
			// Route to appropriate authentication method
			switch method {
			case "", "password":
				return loginWithPassword(ctx, handle)
			case "oauth":
				return loginWithOAuth(ctx, handle)
			case "device":
				return loginWithDevice(ctx, handle)
			default:
				return fmt.Errorf("unsupported authentication method: %s (use password, oauth, or device)", method)
			}
		},
	}
	
	cmd.Flags().String("handle", "", "BlueSky handle (e.g., alice.bsky.social)")
	cmd.Flags().String("method", "password", "Authentication method (password|oauth|device)")
	
	return cmd
}

// loginWithPassword performs password-based authentication
func loginWithPassword(ctx context.Context, handle string) error {
	// Prompt for password
	fmt.Print("App password: ")
	passwordBytes, err := term.ReadPassword(int(syscall.Stdin))
	fmt.Println()
	if err != nil {
		return fmt.Errorf("failed to read password: %w", err)
	}
	password := string(passwordBytes)
	
	// Create credential manager
	cm, err := auth.NewCredentialManager()
	if err != nil {
		return fmt.Errorf("failed to create credential manager: %w", err)
	}
	
	// Store the app password directly as access token
	creds := &auth.Credentials{
		AccessToken:  password,
		RefreshToken: "",
		DPoPKey:      "",
		ExpiresAt:    time.Now().Add(30 * 24 * time.Hour), // 30 days
	}
	
	if err := cm.StoreCredentials(ctx, handle, creds); err != nil {
		return fmt.Errorf("failed to store credentials: %w", err)
	}
	
	// Set as default account
	if err := cm.SetDefaultAccount(ctx, handle); err != nil {
		return fmt.Errorf("failed to set default account: %w", err)
	}
	
	fmt.Printf("✓ Successfully stored credentials for @%s\n", handle)
	fmt.Println("  Credentials stored securely in system keyring")
	
	return nil
}

// loginWithOAuth performs OAuth 2.0 PKCE authorization code flow
func loginWithOAuth(ctx context.Context, handle string) error {
	client := auth.NewOAuthClient()
	
	// Start authorization flow
	req := &auth.AuthorizationRequest{
		Handle:       handle,
		CallbackPort: 8472, // Local callback port
	}
	
	resp, err := client.StartAuthorizationFlow(req)
	if err != nil {
		return fmt.Errorf("failed to start authorization flow: %w", err)
	}
	
	fmt.Println("🔐 OAuth Authorization Required")
	fmt.Println()
	fmt.Printf("  Please open this URL in your browser:\n  %s\n", resp.AuthURL)
	fmt.Println()
	fmt.Print("Waiting for authorization...")
	
	// TODO: Implement local callback server to receive authorization code
	// For now, prompt user to paste the code manually
	fmt.Println()
	fmt.Print("Authorization code: ")
	var code string
	fmt.Scanln(&code)
	
	// Exchange code for tokens
	tokenReq := &auth.TokenRequest{
		Code:         code,
		CodeVerifier: resp.CodeVerifier,
	}
	
	tokens, err := client.ExchangeCodeForToken(ctx, tokenReq)
	if err != nil {
		return fmt.Errorf("failed to exchange code for token: %w", err)
	}
	
	// Store credentials
	cm, err := auth.NewCredentialManager()
	if err != nil {
		return fmt.Errorf("failed to create credential manager: %w", err)
	}
	
	creds := &auth.Credentials{
		AccessToken:  tokens.AccessToken,
		RefreshToken: tokens.RefreshToken,
		DPoPKey:      "", // TODO: Generate DPoP key
		ExpiresAt:    tokens.ExpiresAt,
	}
	
	if err := cm.StoreCredentials(ctx, handle, creds); err != nil {
		return fmt.Errorf("failed to store credentials: %w", err)
	}
	
	if err := cm.SetDefaultAccount(ctx, handle); err != nil {
		return fmt.Errorf("failed to set default account: %w", err)
	}
	
	fmt.Printf("\n✓ Successfully authenticated @%s via OAuth\n", handle)
	fmt.Println("  Credentials stored securely in system keyring")
	
	return nil
}

// loginWithDevice performs device authorization grant flow
func loginWithDevice(ctx context.Context, handle string) error {
	client := auth.NewOAuthClient()
	
	// Start device flow
	req := &auth.DeviceAuthorizationRequest{
		Handle: handle,
	}
	
	device, err := client.StartDeviceFlow(ctx, req)
	if err != nil {
		return fmt.Errorf("failed to start device flow: %w", err)
	}
	
	fmt.Println("🔐 Device Authorization Required")
	fmt.Println()
	fmt.Printf("  1. Visit: %s\n", device.VerificationURI)
	fmt.Printf("  2. Enter code: %s\n", device.UserCode)
	fmt.Println()
	fmt.Println("Waiting for authorization (this may take a few minutes)...")
	
	// Poll for completion
	pollReq := &auth.PollDeviceTokenRequest{
		DeviceCode: device.DeviceCode,
	}
	
	interval := time.Duration(device.Interval) * time.Second
	ticker := time.NewTicker(interval)
	defer ticker.Stop()
	
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			tokens, err := client.PollDeviceToken(ctx, pollReq)
			if err == auth.ErrAuthorizationPending {
				continue // Keep polling
			} else if err == auth.ErrSlowDown {
				// Increase interval
				ticker.Reset(interval + 5*time.Second)
				continue
			} else if err != nil {
				return fmt.Errorf("device authorization failed: %w", err)
			}
			
			// Success! Store credentials
			cm, err := auth.NewCredentialManager()
			if err != nil {
				return fmt.Errorf("failed to create credential manager: %w", err)
			}
			
			creds := &auth.Credentials{
				AccessToken:  tokens.AccessToken,
				RefreshToken: tokens.RefreshToken,
				DPoPKey:      "", // TODO: Generate DPoP key
				ExpiresAt:    tokens.ExpiresAt,
			}
			
			if err := cm.StoreCredentials(ctx, handle, creds); err != nil {
				return fmt.Errorf("failed to store credentials: %w", err)
			}
			
			if err := cm.SetDefaultAccount(ctx, handle); err != nil {
				return fmt.Errorf("failed to set default account: %w", err)
			}
			
			fmt.Printf("\n✓ Successfully authenticated @%s via device flow\n", handle)
			fmt.Println("  Credentials stored securely in system keyring")
			
			return nil
		}
	}
}

// createAccountsCommand creates the accounts list command
func createAccountsCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "accounts",
		Short: "List authenticated accounts",
		Long:  `List all authenticated BlueSky accounts stored in the system keyring.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			cm, err := auth.NewCredentialManager()
			if err != nil {
				return fmt.Errorf("failed to create credential manager: %w", err)
			}
			
			ctx := context.Background()
			accounts, err := cm.ListAccounts(ctx)
			if err != nil {
				return fmt.Errorf("failed to list accounts: %w", err)
			}
			
			if len(accounts) == 0 {
				fmt.Println("No authenticated accounts found.")
				fmt.Println("Run 'autoreply login' to authenticate.")
				return nil
			}
			
			defaultAccount, err := cm.GetDefaultAccount(ctx)
			if err != nil {
				return fmt.Errorf("failed to get default account: %w", err)
			}
			
			fmt.Println("Authenticated Accounts:")
			for _, account := range accounts {
				marker := " "
				if defaultAccount != nil && *defaultAccount == account.Handle {
					marker = "✓"
				}
				
				fmt.Printf("  %s %s\n", marker, account.Handle)
				if account.DID != "" {
					fmt.Printf("    DID: %s\n", account.DID)
				}
				if account.PDS != "" {
					fmt.Printf("    PDS: %s\n", account.PDS)
				}
				fmt.Printf("    Created: %s\n", account.CreatedAt.Format(time.RFC3339))
				fmt.Printf("    Last used: %s\n", account.LastUsed.Format(time.RFC3339))
				
				if marker == "✓" {
					fmt.Println("    (default)")
				}
				fmt.Println()
			}
			
			return nil
		},
	}
}

// createLogoutCommand creates the logout command
func createLogoutCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "logout <handle>",
		Short: "Remove stored credentials",
		Long:  `Remove stored credentials for a BlueSky account from the system keyring.`,
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			handle := args[0]
			
			cm, err := auth.NewCredentialManager()
			if err != nil {
				return fmt.Errorf("failed to create credential manager: %w", err)
			}
			
			ctx := context.Background()
			if err := cm.DeleteCredentials(ctx, handle); err != nil {
				return fmt.Errorf("failed to delete credentials: %w", err)
			}
			
			fmt.Printf("✓ Logged out from @%s\n", handle)
			fmt.Println("  Credentials removed from system keyring")
			
			return nil
		},
	}
}

// createUseCommand creates the use/set-default command
func createUseCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "use <handle>",
		Short: "Set default account",
		Long:  `Set the default BlueSky account to use for operations.`,
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			handle := args[0]
			
			cm, err := auth.NewCredentialManager()
			if err != nil {
				return fmt.Errorf("failed to create credential manager: %w", err)
			}
			
			ctx := context.Background()
			if err := cm.SetDefaultAccount(ctx, handle); err != nil {
				return fmt.Errorf("failed to set default account: %w", err)
			}
			
			fmt.Printf("✓ Default account set to @%s\n", handle)
			
			return nil
		},
	}
}
