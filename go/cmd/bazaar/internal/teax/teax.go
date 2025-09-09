package teax

import tea "github.com/charmbracelet/bubbletea"

func SwitchModel(m tea.Model) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.Init(), tea.WindowSize())
}
