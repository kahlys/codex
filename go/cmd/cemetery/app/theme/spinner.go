// Package theme defines shared styles and widgets for the cemetery app.
package theme

import "github.com/charmbracelet/bubbles/spinner"

// NewSpinner returns the default spinner used by the app.
func NewSpinner() spinner.Model {
	s := spinner.New()
	s.Spinner = spinner.Line
	return s
}
