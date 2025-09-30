// Package cli provides command-line interface support for trial mode
package cli

import (
	"context"
	"encoding/json"
	"reflect"

	"github.com/invopop/jsonschema"
	"github.com/oyin-bo/autoreply/go-server/internal/mcp"
	"github.com/spf13/cobra"
)

// ToolDefinition defines a tool with its metadata and execution logic
type ToolDefinition struct {
	Name        string
	Description string
	ArgsType    interface{}
	Execute     func(ctx context.Context, args interface{}) (string, error)
}

// Registry manages all available tools
type Registry struct {
	tools map[string]*ToolDefinition
}

// NewRegistry creates a new tool registry
func NewRegistry() *Registry {
	return &Registry{
		tools: make(map[string]*ToolDefinition),
	}
}

// RegisterTool adds a tool to the registry
func (r *Registry) RegisterTool(def *ToolDefinition) {
	r.tools[def.Name] = def
}

// GetTool retrieves a tool by name
func (r *Registry) GetTool(name string) (*ToolDefinition, bool) {
	tool, exists := r.tools[name]
	return tool, exists
}

// ExtractMCPInputSchema generates MCP JSON Schema from the args type
func (td *ToolDefinition) ExtractMCPInputSchema() mcp.InputSchema {
	reflector := &jsonschema.Reflector{
		DoNotReference:             true,
		ExpandedStruct:             true,
		AllowAdditionalProperties:  false,
		RequiredFromJSONSchemaTags: true,
	}

	schema := reflector.Reflect(td.ArgsType)

	// Convert jsonschema.Schema to mcp.InputSchema
	properties := make(map[string]mcp.PropertySchema)
	required := []string{}

	if schema.Properties != nil {
		// Iterate through properties using the ordered map
		for pair := schema.Properties.Oldest(); pair != nil; pair = pair.Next() {
			propName := pair.Key
			propSchema := pair.Value

			properties[propName] = mcp.PropertySchema{
				Type:        propSchema.Type,
				Description: propSchema.Description,
			}
		}
	}

	if len(schema.Required) > 0 {
		required = schema.Required
	}

	return mcp.InputSchema{
		Type:       "object",
		Properties: properties,
		Required:   required,
	}
}

// CreateCobraCommand generates a cobra command from a tool definition
func CreateCobraCommand(td *ToolDefinition, execute func(ctx context.Context, args interface{}) error) *cobra.Command {
	cmd := &cobra.Command{
		Use:   td.Name,
		Short: td.Description,
		Long:  td.Description,
		RunE: func(cmd *cobra.Command, args []string) error {
			ctx := cmd.Context()
			argsInstance := createArgsInstance(td.ArgsType)
			
			// Bind flags to struct fields
			if err := bindFlagsToStruct(cmd, argsInstance); err != nil {
				return err
			}

			return execute(ctx, argsInstance)
		},
	}

	// Add flags based on struct fields
	addFlagsFromStruct(cmd, td.ArgsType)

	return cmd
}

// createArgsInstance creates a new instance of the args type
func createArgsInstance(argsType interface{}) interface{} {
	t := reflect.TypeOf(argsType)
	if t.Kind() == reflect.Ptr {
		t = t.Elem()
	}
	return reflect.New(t).Interface()
}

// addFlagsFromStruct adds cobra flags based on struct tags
func addFlagsFromStruct(cmd *cobra.Command, argsType interface{}) {
	t := reflect.TypeOf(argsType)
	if t.Kind() == reflect.Ptr {
		t = t.Elem()
	}

	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		
		shortFlag := field.Tag.Get("short")
		longFlag := field.Tag.Get("long")
		description := getDescription(field)

		if longFlag == "" {
			continue
		}

		switch field.Type.Kind() {
		case reflect.String:
			if shortFlag != "" {
				cmd.Flags().StringP(longFlag, shortFlag, "", description)
			} else {
				cmd.Flags().String(longFlag, "", description)
			}
		case reflect.Int:
			if shortFlag != "" {
				cmd.Flags().IntP(longFlag, shortFlag, 0, description)
			} else {
				cmd.Flags().Int(longFlag, 0, description)
			}
		}

		// Mark required fields
		if hasRequiredTag(field) {
			cmd.MarkFlagRequired(longFlag)
		}
	}
}

// bindFlagsToStruct binds cobra flag values to struct fields
func bindFlagsToStruct(cmd *cobra.Command, argsInstance interface{}) error {
	v := reflect.ValueOf(argsInstance)
	if v.Kind() == reflect.Ptr {
		v = v.Elem()
	}

	t := v.Type()
	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		longFlag := field.Tag.Get("long")
		
		if longFlag == "" {
			continue
		}

		switch field.Type.Kind() {
		case reflect.String:
			val, err := cmd.Flags().GetString(longFlag)
			if err != nil {
				return err
			}
			v.Field(i).SetString(val)
		case reflect.Int:
			val, err := cmd.Flags().GetInt(longFlag)
			if err != nil {
				return err
			}
			v.Field(i).SetInt(int64(val))
		}
	}

	return nil
}

// getDescription extracts description from jsonschema tag
func getDescription(field reflect.StructField) string {
	tag := field.Tag.Get("jsonschema")
	if tag == "" {
		return ""
	}

	// Parse jsonschema tag for description
	// Format: "required,description=Some description"
	parts := parseTag(tag)
	if desc, ok := parts["description"]; ok {
		return desc
	}
	return ""
}

// hasRequiredTag checks if field is required
func hasRequiredTag(field reflect.StructField) bool {
	tag := field.Tag.Get("jsonschema")
	if tag == "" {
		return false
	}
	parts := parseTag(tag)
	_, required := parts["required"]
	return required
}

// parseTag parses a jsonschema tag into key-value pairs
func parseTag(tag string) map[string]string {
	result := make(map[string]string)
	
	// Simple parser for comma-separated tags
	for _, part := range splitTag(tag) {
		if part == "required" {
			result["required"] = "true"
			continue
		}
		
		// Handle key=value pairs
		if idx := findEquals(part); idx != -1 {
			key := part[:idx]
			value := part[idx+1:]
			result[key] = value
		}
	}
	
	return result
}

// splitTag splits a tag by commas, respecting quoted strings
func splitTag(tag string) []string {
	var parts []string
	var current []rune
	inQuotes := false
	
	for _, ch := range tag {
		switch ch {
		case ',':
			if !inQuotes && len(current) > 0 {
				parts = append(parts, string(current))
				current = nil
			} else {
				current = append(current, ch)
			}
		case '"', '\'':
			inQuotes = !inQuotes
		default:
			current = append(current, ch)
		}
	}
	
	if len(current) > 0 {
		parts = append(parts, string(current))
	}
	
	return parts
}

// findEquals finds the first '=' character not in quotes
func findEquals(s string) int {
	for i, ch := range s {
		if ch == '=' {
			return i
		}
	}
	return -1
}

// ConvertToMap converts a struct to a map for MCP tool calls
func ConvertToMap(args interface{}) (map[string]interface{}, error) {
	data, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, err
	}

	return result, nil
}
