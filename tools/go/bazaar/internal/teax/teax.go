// Package teax provides Bubble Tea helpers.
package teax

import tea "charm.land/bubbletea/v2"

// SwitchModel swaps to a new model and requests its init and a window size refresh.
func SwitchModel(m tea.Model) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.Init(), tea.RequestWindowSize)
}

// View wraps a string into a tea.View with AltScreen set to true.
func View(content string) tea.View {
	return tea.View{
		Content:   content,
		AltScreen: true,
	}
}
