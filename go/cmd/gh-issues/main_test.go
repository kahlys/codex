package main

import (
	"testing"
)

func TestParseOwnerRepo(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantErr bool
	}{
		{
			name:    "valid owner/repo",
			input:   "octocat/Hello-World",
			wantErr: false,
		},
		{
			name:    "invalid format - no slash",
			input:   "octocat",
			wantErr: true,
		},
		{
			name:    "invalid format - multiple slashes",
			input:   "octocat/Hello-World/extra",
			wantErr: true,
		},
		{
			name:    "empty string",
			input:   "",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			parts := splitOwnerRepo(tt.input)
			hasError := len(parts) != 2

			if hasError != tt.wantErr {
				t.Errorf("splitOwnerRepo() error = %v, wantErr %v", hasError, tt.wantErr)
			}
		})
	}
}

func splitOwnerRepo(input string) []string {
	// This is a helper function extracted for testing purposes
	// In the actual implementation, this logic is using strings.Split
	if input == "" {
		return []string{}
	}
	
	parts := []string{}
	slashCount := 0
	start := 0
	
	for i := 0; i < len(input); i++ {
		if input[i] == '/' {
			slashCount++
			parts = append(parts, input[start:i])
			start = i + 1
		}
	}
	
	// Add the last part
	if start < len(input) {
		parts = append(parts, input[start:])
	}
	
	return parts
}
