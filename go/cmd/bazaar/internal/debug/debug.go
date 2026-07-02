// Package debug contains debug utilities for the bazaar CLI.
package debug

import (
	"log"
	"os"
)

var logger *log.Logger

func init() {
	file, err := os.OpenFile("debug.log", os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		log.Fatalf("failed to open debug.log: %v", err)
	}
	logger = log.New(file, "DEBUG: ", log.Ldate|log.Ltime|log.Lshortfile)
}

// Log writes a debug message to debug.log
func Log(v ...interface{}) {
	logger.Println(v...)
}

// Logf writes a formatted debug message to debug.log
func Logf(format string, v ...interface{}) {
	logger.Printf(format, v...)
}
