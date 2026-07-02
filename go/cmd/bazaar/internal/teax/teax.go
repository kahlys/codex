// Package teax provides Bubble Tea helpers.
package teax

import tea "github.com/charmbracelet/bubbletea"

// SwitchModel swaps to a new model and requests its init and a window size refresh.
func SwitchModel(m tea.Model) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.Init(), tea.WindowSize())
}
