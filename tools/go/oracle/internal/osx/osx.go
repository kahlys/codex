// Package osx provides file helpers used by the oracle tool.
package osx

import (
	"io"
	"os"
	"path/filepath"
)

// IsFileExist returns true if the file exists.
func IsFileExist(filePath string) bool {
	_, err := os.Stat(filePath)
	return !os.IsNotExist(err)
}

// NewTempFile creates a temporary file and returns its path.
func NewTempFile() (string, error) {
	tempFile, err := os.CreateTemp("", "tempfile_")
	if err != nil {
		return "", err
	}
	tempFile.Close()

	tempFilePath, err := filepath.Abs(tempFile.Name())
	if err != nil {
		return "", err
	}

	return tempFilePath, nil
}

// CopyFile copies a file to a temporary file and returns its path.
func CopyFile(pathSourceFile string, pathDestFile string) error {
	sourceFile, err := os.Open(pathSourceFile)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	destFile, err := os.Create(pathDestFile)
	if err != nil {
		return err
	}
	defer destFile.Close()

	_, err = io.Copy(destFile, sourceFile)
	if err != nil {
		return err
	}

	err = destFile.Sync()
	if err != nil {
		return err
	}

	return nil
}
