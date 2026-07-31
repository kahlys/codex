// Package keys defines key bindings for the cemetery app.
package keys

import "charm.land/bubbles/v2/key"

// DefaultKeys are the default keybindings for the app.
type DefaultKeys struct {
	Enter key.Binding
	Nav   key.Binding
	Quit  key.Binding
}

// NewDefaultKeys returns a new DefaultKeys.
func NewDefaultKeys() DefaultKeys {
	return DefaultKeys{
		Enter: key.NewBinding(key.WithKeys("enter"), key.WithHelp("enter", "select")),
		Nav:   key.NewBinding(key.WithKeys("ctrl+n"), key.WithHelp("ctrl+n", "nav")),
		Quit:  key.NewBinding(key.WithKeys("ctrl+c"), key.WithHelp("ctrl+c", "quit")),
	}
}

// ShortHelp returns keybindings to be shown in the mini help view.
func (k DefaultKeys) ShortHelp() []key.Binding {
	return []key.Binding{k.Nav, k.Quit}
}

// FullHelp returns keybindings for the expanded help view.
func (k DefaultKeys) FullHelp() [][]key.Binding {
	return [][]key.Binding{
		{k.Nav, k.Quit},
	}
}
