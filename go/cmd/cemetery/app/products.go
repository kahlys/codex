// Package app contains Bubble Tea models for the cemetery TUI.
package app

import (
	"fmt"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/lipgloss/table"

	"github.com/kahlys/codex/go/cmd/cemetery/api"
	"github.com/kahlys/codex/go/cmd/cemetery/app/keys"
	"github.com/kahlys/codex/go/cmd/cemetery/app/message"
	"github.com/kahlys/codex/go/cmd/cemetery/app/theme"
)

// HomeModel renders and drives the products list view.
type HomeModel struct {
	help help.Model
	keys keys.DefaultKeys

	list list.Model
	err  message.Error
}

// NewHomeModel creates the products list model.
func NewHomeModel() tea.Model {
	list := list.New(
		[]list.Item{},
		list.NewDefaultDelegate(), 0, 0,
	)
	list.Title = "Products"

	return &HomeModel{
		help: help.New(),
		keys: keys.NewDefaultKeys(),
		list: list,
	}
}

// Init loads products for the home view.
func (m *HomeModel) Init() tea.Cmd {
	return tea.Batch(
		func() tea.Msg {
			res, err := api.ListProducts()
			if err != nil {
				return message.NewError(err)
			}
			return res
		},
	)
}

func productsToListItems(products []api.Product) []list.Item {
	items := make([]list.Item, len(products))
	for i := range products {
		items[i] = products[i]
	}
	return items
}

// Update handles messages for the home view.
func (m *HomeModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch {
		case key.Matches(msg, m.keys.Quit):
			return m, tea.Quit
		case key.Matches(msg, m.keys.Enter):
			if m.list.FilterState() != list.Filtering {
				return goToNewModel(NewProductModel(m.list.SelectedItem().(api.Product).URI))
			}
		}

	case api.Products:
		m.list.SetItems(productsToListItems(msg.Result))

	case message.Error:
		m.err = msg
		return m, nil

	case tea.WindowSizeMsg:
		h, v := theme.ContainerFrameSize()
		m.list.SetSize(msg.Width-h, msg.Height-v)
	}

	var cmd tea.Cmd
	m.list, cmd = m.list.Update(msg)
	return m, cmd
}

// View renders the home view.
func (m *HomeModel) View() string {
	if m.err != (message.Error{}) {
		return m.err.Error()
	}
	return theme.Container.Render(m.list.View())
}

// ProductModel displays releases for a selected product.
type ProductModel struct {
	uri string

	help    help.Model
	keys    keys.DefaultKeys
	spinner spinner.Model

	response api.ProductInfo
	err      message.Error
}

// NewProductModel creates a product details model.
func NewProductModel(uri string) tea.Model {
	return &ProductModel{
		uri:     uri,
		help:    help.New(),
		spinner: theme.NewSpinner(),
		keys:    keys.NewDefaultKeys(),
	}
}

// Init starts spinner updates and fetches product details.
func (m *ProductModel) Init() tea.Cmd {
	return tea.Batch(
		m.spinner.Tick,
		func() tea.Msg {
			res, err := api.GetProductWithURI(m.uri)
			if err != nil {
				return message.NewError(err)
			}
			return res
		},
	)
}

// Update handles messages for the product details view.
func (m *ProductModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch {
		case key.Matches(msg, m.keys.Quit):
			return m, tea.Quit
		case key.Matches(msg, m.keys.Nav):
			return goToNewModel(NewHomeModel())
		}

	case api.ProductInfo:
		m.response = msg
		return m, nil

	case message.Error:
		m.err = msg
		return m, nil

	default:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	}

	return m, nil
}

func dateWithDanger(sign bool, date string) string {
	if !sign {
		return date
	}
	return fmt.Sprintf("%s ❌", date)
}

func productToTableRows(product api.ProductInfo, limit int) [][]string {
	rows := [][]string{}

	for i, r := range product.Result.Releases {
		if i >= limit {
			break
		}
		rows = append(
			rows,
			[]string{
				r.Name,
				r.ReleaseDate,
				dateWithDanger(r.IsEoas, r.EoasFrom),
				dateWithDanger(r.IsEol, r.EolFrom),
				dateWithDanger(r.IsEoes, r.EoesFrom),
				fmt.Sprintf("%v (%v)", r.Latest.Name, r.Latest.Date),
			},
		)
	}

	return rows
}

// View renders the product details table.
func (m *ProductModel) View() string {
	res := theme.Title.Render(m.response.Result.Label) + "\n\n"

	t := table.New().
		Border(lipgloss.NormalBorder()).
		BorderStyle(lipgloss.NewStyle().Foreground(theme.Color1)).
		Headers(
			"Release",
			"Released",
			m.response.Result.Labels.Eoas,
			m.response.Result.Labels.Eol,
			m.response.Result.Labels.Eoes,
			"Latest",
		).
		Rows(productToTableRows(m.response, 20)...)

	res += t.Render() + "\n\n" + m.help.View(m.keys)

	return theme.Container.Render(res)
}

func goToNewModel(m tea.Model) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.Init(), tea.WindowSize())
}
