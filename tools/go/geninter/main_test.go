package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func Test_run(t *testing.T) {
	pkgPath := "testpkg"
	structName := "User"

	want, err := os.ReadFile(filepath.Join(pkgPath, "expected"))
	require.NoError(t, err)

	got, err := run(pkgPath, structName)
	require.NoError(t, err)
	require.Equal(t, string(want), got)
}
