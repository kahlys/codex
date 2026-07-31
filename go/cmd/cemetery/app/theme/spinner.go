// Package theme defines shared styles and widgets for the cemetery app.
package theme

import "charm.land/bubbles/v2/spinner"

// NewSpinner returns the default spinner used by the app.
func NewSpinner() spinner.Model {
	s := spinner.New()
	s.Spinner = spinner.Line
	return s
}
