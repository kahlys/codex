package theme

import "github.com/charmbracelet/bubbles/spinner"

func NewSpinner() spinner.Model {
	s := spinner.New()
	s.Spinner = spinner.Line
	return s
}
