package main

import (
	"log"
	"os"

	tea "github.com/charmbracelet/bubbletea"

	"github.com/kahlys/codex/go/cmd/cemetery/app"
)

func main() {
	if len(os.Getenv("DEBUG")) > 0 {
		f, err := tea.LogToFile("debug.log", "debug")
		if err != nil {
			os.Exit(1)
		}
		defer f.Close()
	}

	if _, err := tea.NewProgram(
		app.NewHomeModel(),
		tea.WithAltScreen(),
	).Run(); err != nil {
		log.Fatal(err)
	}
}
