package theme

import "github.com/charmbracelet/lipgloss"

// Container is the outer layout style for views.
var Container = lipgloss.NewStyle().Margin(1, 2)

var (
	// Color1 is the primary accent color.
	Color1 = lipgloss.Color("62")
	// Color2 is the secondary accent color.
	Color2 = lipgloss.AdaptiveColor{Light: "#EE6FF8", Dark: "#EE6FF8"}
)

var (
	// Title is the heading style.
	Title = lipgloss.NewStyle().
		Background(Color1).
		PaddingLeft(1).
		PaddingRight(1).
		Bold(true)

	// SubTitle is the style used for section subtitles.
	SubTitle = lipgloss.NewStyle().
			Foreground(Color2).
			Bold(true).
			Underline(true)
)

// ContainerFrameSize returns the sum of the margins, padding and border width for
// both the horizontal and vertical margins.
func ContainerFrameSize() (int, int) {
	return Container.GetFrameSize()
}
