package cli

import (
	"context"
	"fmt"
	"os/exec"
	"runtime"
	"syscall"
	"time"

	"github.com/oyin-bo/autoreply/go-server/internal/auth"
	"github.com/oyin-bo/autoreply/go-server/internal/bluesky"
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
				return fmt.Errorf("unsupported authentication method: %s\n" +
					"Supported methods:\n" +
					"  password - App password authentication (recommended)\n" +
					"  oauth    - OAuth PKCE flow (not yet fully implemented)\n" +
					"  device   - Device authorization (not yet fully implemented)", method)
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
	fmt.Printf("Starting OAuth PKCE flow for @%s...\n", handle)
	
	// Step 1: Resolve handle to DID and discover PDS
	fmt.Println("⏳ Resolving handle to DID and discovering PDS...")
	
	// Import DID resolver
	didResolver := bluesky.NewDIDResolver()
	did, err := didResolver.ResolveHandle(ctx, handle)
	if err != nil {
		return fmt.Errorf("failed to resolve handle to DID: %w", err)
	}
	fmt.Printf("✓ Resolved DID: %s\n", did)
	
	pds, err := didResolver.ResolvePDSEndpoint(ctx, did)
	if err != nil {
		return fmt.Errorf("failed to discover PDS: %w", err)
	}
	if pds == "" {
		return fmt.Errorf("no PDS endpoint found for user")
	}
	fmt.Printf("✓ Discovered PDS: %s\n", pds)
	
	// Step 2: Discover OAuth metadata
	fmt.Println("⏳ Discovering OAuth server metadata...")
	client := auth.NewAtProtoOAuthClient("autoreply-mcp-client")
	metadata, err := client.DiscoverMetadata(ctx, pds)
	if err != nil {
		return fmt.Errorf("failed to discover OAuth metadata: %w", err)
	}
	fmt.Printf("✓ OAuth server: %s\n", metadata.Issuer)
	
	// Step 3: Generate PKCE and DPoP key pair
	fmt.Println("⏳ Generating PKCE challenge and DPoP key pair...")
	pkce, err := auth.GeneratePKCE()
	if err != nil {
		return fmt.Errorf("failed to generate PKCE parameters: %w", err)
	}
	
	dpopKeyPair, err := auth.GenerateDPoPKeyPair()
	if err != nil {
		return fmt.Errorf("failed to generate DPoP key pair: %w", err)
	}
	fmt.Println("✓ Generated security parameters")
	
	// Step 4: Send PAR (Pushed Authorization Request)
	fmt.Println("⏳ Sending Pushed Authorization Request...")
	parResponse, err := client.SendPAR(ctx, metadata, pkce, dpopKeyPair, handle)
	if err != nil {
		return fmt.Errorf("failed to send PAR: %w", err)
	}
	fmt.Printf("✓ Received request URI (expires in %ds)\n", parResponse.ExpiresIn)
	
	// Step 5: Build authorization URL
	authURL := client.BuildAuthorizationURL(metadata, parResponse)
	
	// Step 6: Start callback server
	fmt.Println("⏳ Starting OAuth callback server on port 8080...")
	callbackServer := auth.NewOAuthCallbackServer(8080)
	
	// Step 7: Open browser
	fmt.Println("\n╔══════════════════════════════════════════════════════════════╗")
	fmt.Println("║  Please authorize in your browser:                          ║")
	fmt.Println("╠══════════════════════════════════════════════════════════════╣")
	fmt.Printf("║  %s  ║\n", authURL)
	fmt.Println("╚══════════════════════════════════════════════════════════════╝")
	fmt.Println()
	
	// Try to open browser
	if err := openBrowser(authURL); err != nil {
		fmt.Printf("⚠  Could not open browser automatically: %v\n", err)
		fmt.Println("   Please open the URL above manually.")
	}
	
	// Step 8: Wait for callback with timeout
	callbackCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	
	fmt.Println("⏳ Waiting for authorization callback...")
	callbackResult, err := callbackServer.WaitForCallback(callbackCtx)
	if err != nil {
		return fmt.Errorf("failed to receive OAuth callback: %w", err)
	}
	fmt.Println("✓ Received authorization code")
	
	// Step 9: Exchange code for tokens
	fmt.Println("⏳ Exchanging authorization code for access token...")
	tokens, err := client.ExchangeCodeForTokens(
		ctx,
		metadata,
		callbackResult.Code,
		pkce.CodeVerifier,
		dpopKeyPair,
		"http://127.0.0.1:8080/callback",
	)
	if err != nil {
		return fmt.Errorf("failed to exchange code for tokens: %w", err)
	}
	fmt.Printf("✓ Received access token (expires in %ds)\n", tokens.ExpiresIn)
	
	// Step 10: Store credentials
	fmt.Println("⏳ Storing credentials securely...")
	cm, err := auth.NewCredentialManager()
	if err != nil {
		return fmt.Errorf("failed to create credential manager: %w", err)
	}
	
	dpopKeyPEM, err := dpopKeyPair.ToPEM()
	if err != nil {
		return fmt.Errorf("failed to export DPoP key: %w", err)
	}
	
	creds := &auth.Credentials{
		AccessToken:  tokens.AccessToken,
		RefreshToken: tokens.RefreshToken,
		DPoPKey:      dpopKeyPEM,
		ExpiresAt:    tokens.ExpiresAt,
	}
	
	if err := cm.StoreCredentials(ctx, handle, creds); err != nil {
		return fmt.Errorf("failed to store credentials: %w", err)
	}
	
	if err := cm.SetDefaultAccount(ctx, handle); err != nil {
		return fmt.Errorf("failed to set default account: %w", err)
	}
	
	fmt.Printf("✓ Successfully authenticated @%s\n", handle)
	fmt.Printf("  DID: %s\n", did)
	fmt.Printf("  PDS: %s\n", pds)
	fmt.Println("  Credentials stored securely in system keyring")
	
	return nil
}

// openBrowser opens a URL in the default browser
func openBrowser(url string) error {
	var cmd string
	var args []string
	
	switch runtime.GOOS {
	case "windows":
		cmd = "cmd"
		args = []string{"/c", "start"}
	case "darwin":
		cmd = "open"
	default: // "linux", "freebsd", "openbsd", "netbsd"
		cmd = "xdg-open"
	}
	args = append(args, url)
	return exec.Command(cmd, args...).Start()
}

// loginWithDevice performs device authorization grant flow
func loginWithDevice(ctx context.Context, handle string) error {
	// NOTE: Full AT Protocol Device Flow implementation requires:
	// 1. DID resolution and PDS discovery for the handle
	// 2. Device authorization endpoint discovery
	// 3. DPoP proof generation for token requests
	// 4. Proper polling with OAuth server metadata
	//
	// The current implementation provides basic device flow primitives but
	// does not implement the complete AT Protocol device authorization.
	// For production use, use app passwords until full implementation.
	
	return fmt.Errorf("Device authorization flow not yet fully implemented for AT Protocol.\n" +
		"AT Protocol device flow requires additional components:\n" +
		"  - DID resolution and PDS discovery\n" +
		"  - Device authorization endpoint discovery\n" +
		"  - DPoP proof generation\n" +
		"  - OAuth metadata discovery\n\n" +
		"Please use --method password (app passwords) for now.\n" +
		"See docs/12-auth-implementation-plan.md for implementation details.")
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
