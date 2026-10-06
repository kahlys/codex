//go:build !windows

// Package webstealer extracts credentials from browser storage.
package webstealer

// ChromeStealer reads and decrypts credentials stored by Chrome.
type ChromeStealer struct{}

// NewChromeStealer builds a ChromeStealer. Unsupported on non-Windows platforms.
func NewChromeStealer() ChromeStealer {
	return ChromeStealer{}
}

// Run extracts stored Chrome credentials. Unsupported on non-Windows platforms.
func (cs ChromeStealer) Run() (Results, error) {
	return Results{}, nil
}
