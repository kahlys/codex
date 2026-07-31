// Package main runs the bazaar TUI for local container management.
package main

import (
	"log"

	tea "charm.land/bubbletea/v2"

	"github.com/kahlys/codex/go/cmd/bazaar/model"
)

func main() {
	if _, err := tea.NewProgram(
		model.NewContainersModel(),
	).Run(); err != nil {
		log.Fatal(err)
	}
}
