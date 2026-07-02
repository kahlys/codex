// Package main runs the bazaar TUI for local container management.
package main

import (
	"log"

	tea "github.com/charmbracelet/bubbletea"

	"github.com/kahlys/codex/go/cmd/bazaar/model"
)

func main() {
	if _, err := tea.NewProgram(
		model.NewContainersModel(),
		tea.WithAltScreen(),
	).Run(); err != nil {
		log.Fatal(err)
	}
}
