package model

import (
	"github.com/charmbracelet/lipgloss"
)

var (
	doc = lipgloss.NewStyle().Margin(1, 2)

	Color1 = lipgloss.Color("62")

	Border = lipgloss.NewStyle().Border(lipgloss.NormalBorder())

	Title = lipgloss.NewStyle().
		Background(Color1).
		PaddingLeft(1).
		PaddingRight(1).
		Margin(1).
		Bold(true)
)

func borderSize() (int, int) {
	return Border.GetFrameSize()
}

func titleView() string {
	return Title.Render("Bazaar")
}
