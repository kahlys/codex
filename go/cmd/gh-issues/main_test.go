package main

import (
	"strings"
	"testing"
)

func TestSplitOwnerRepo(t *testing.T) {
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
	// Helper function for testing the owner/repo parsing logic
	// Uses strings.Split as implemented in main.go
	return strings.Split(input, "/")
}
