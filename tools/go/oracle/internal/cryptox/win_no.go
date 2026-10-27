//go:build !windows

package cryptox

import (
	"fmt"
)

// WDecrypt is unsupported on non-Windows platforms.
func WDecrypt(_ []byte) ([]byte, error) {
	return nil, fmt.Errorf("only available on windows")
}
