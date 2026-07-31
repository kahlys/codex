// Package model contains Bubble Tea models for the bazaar CLI.
package model

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/list"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/huh"
	"github.com/charmbracelet/lipgloss"

	"github.com/kahlys/codex/go/cmd/bazaar/internal/dockerx"
	"github.com/kahlys/codex/go/cmd/bazaar/internal/teax"
)

// ContainersModel is the main model for managing Docker containers.
type ContainersModel struct {
	list list.Model
	keys containerKeyMap
	help help.Model

	deleteState deleteState
	deleteForm  *huh.Form

	width      int
	height     int
	infoWidth  int
	infoHeight int
}

type deleteState int

const (
	inactive deleteState = iota
	deleting
)

// NewContainersModel creates a new ContainersModel.
func NewContainersModel() ContainersModel {
	return ContainersModel{
		list:        containerList(),
		deleteState: inactive,
		help:        help.New(),
		keys:        newContainerKeyMap(),
	}
}

// containerList creates a list model populated with the current containers.
func containerList() list.Model {
	l := list.New(containerListItems(), list.NewDefaultDelegate(), 0, 0)
	l.Title = "Managed Containers"
	l.SetShowStatusBar(false)
	l.SetFilteringEnabled(false)
	l.SetShowHelp(false)
	l.SetShowTitle(false)
	return l
}

// containerListItems converts containerItems to a slice of list.Item for use in the list model.
func containerListItems() []list.Item {
	containers := containers()

	items := make([]list.Item, len(containers))
	for i, c := range containers {
		items[i] = c
	}
	return items
}

// refreshPeriod defines how often the container list is refreshed.
const refreshPeriod = 5 * time.Second

// Init initializes the ContainersModel.
func (m ContainersModel) Init() tea.Cmd {
	return tickCmd(refreshPeriod)
}

const divider = 3

// Update handles incoming messages and updates the model accordingly.
func (m ContainersModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.KeyMsg:
		if key.Matches(msg, m.keys.Quit) {
			return m, tea.Quit
		}

	case tea.WindowSizeMsg:
		titleHeight := lipgloss.Height(titleView())
		helpHeight := lipgloss.Height(m.helpView())

		m.width = msg.Width
		m.height = msg.Height - helpHeight - titleHeight
		horz, vert := doc.GetFrameSize()
		m.list.SetSize((m.width-horz)/divider, m.height-vert)
		m.infoWidth = m.width - m.list.Width() - horz
		m.infoHeight = m.list.Height()

		return m, nil

	case tickMsg:
		m.list.SetItems(containerListItems())
		return m, tickCmd(refreshPeriod)
	}

	switch m.deleteState {
	case inactive:
		return m.handleContainers(msg)
	case deleting:
		return m.handleModal(msg)
	}

	return m, nil
}

// handleContainers processes messages when no modal dialog is active.
func (m ContainersModel) handleContainers(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd

	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch {
		case key.Matches(msg, m.keys.Create):
			return teax.SwitchModel(NewCreateModel())

		case key.Matches(msg, m.keys.Delete):
			m.deleteForm = huh.NewForm(
				huh.NewGroup(
					huh.NewConfirm().
						Key("confirm").
						Title(fmt.Sprintf("Delete container %v ?", m.list.SelectedItem().FilterValue())),
				),
			).WithWidth(40)
			m.deleteState = deleting
			return m, m.deleteForm.Init()

		case key.Matches(msg, m.keys.StartStop):
			if c, ok := m.list.SelectedItem().(container); ok {
				return m, startStopContainer(c.ID, c.State)
			}
		}

	case startStopMsg:
		m.list.SetItems(containerListItems())
		return m, nil
	}

	m.list, cmd = m.list.Update(msg)
	return m, cmd
}

// handleModal processes messages when a modal dialog is active.
func (m ContainersModel) handleModal(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	modalModel, cmd := m.deleteForm.Update(msg)
	if form, ok := modalModel.(*huh.Form); ok {
		m.deleteForm = form
		if m.deleteForm.State == huh.StateCompleted {
			if m.deleteForm.GetBool("confirm") {
				if sel := m.list.SelectedItem(); sel != nil {
					if c, ok := sel.(container); ok {
						_ = dockerx.ContainerStopRemove(context.TODO(), c.ID)
					}
				}
				m.list.SetItems(containerListItems())
			}
			m.deleteForm = nil
			m.deleteState = inactive
		}
	}
	return m, cmd
}

// startStopContainer starts or stops a container based on its current state.
func startStopContainer(id, state string) tea.Cmd {
	return func() tea.Msg {
		var err error
		ctx := context.Background()
		if state == "running" {
			err = dockerx.ContainerStop(ctx, id)
		} else {
			err = dockerx.ContainerStart(ctx, id)
		}
		return startStopMsg{err: err}
	}
}

// View renders the ContainersModel view.
func (m ContainersModel) View() string {
	if m.deleteState == deleting {
		return m.deleteView()
	}
	return m.listView()
}

func (m ContainersModel) deleteView() string {
	return lipgloss.NewStyle().
		Width(44).
		Height(7).
		Border(lipgloss.RoundedBorder()).
		Align(lipgloss.Center).
		MarginTop((m.height - 7) / 2).
		MarginLeft((m.width - 44) / 2).
		Render(m.deleteForm.View())
}

func (m ContainersModel) listView() string {
	detail := "No container selected."
	if sel := m.list.SelectedItem(); sel != nil {
		if c, ok := sel.(container); ok {
			detail = containerInfoView(c)
		}
	}

	return lipgloss.JoinVertical(
		lipgloss.Left,
		titleView(),
		lipgloss.JoinHorizontal(
			lipgloss.Top,
			Border.
				Height(m.list.Height()).
				Width(m.list.Width()).
				Render(m.list.View()),
			Border.
				Height(m.infoHeight).
				Width(m.infoWidth).
				Render(detail),
		),
		m.helpView(),
	)
}

// containerInfoView formats detailed information about a container.
func containerInfoView(c container) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Name: %s\n", c.Name)
	fmt.Fprintf(&b, "ID: %s\n", c.ID[:12])
	fmt.Fprintf(&b, "Image: %s\n", c.Image)
	fmt.Fprintf(&b, "State: %s\n", c.State)
	fmt.Fprintf(&b, "Status: %s\n", c.Status)
	fmt.Fprintf(&b, "Created: %s\n", c.Created.Format(time.DateTime))
	if len(c.Ports) > 0 {
		fmt.Fprintf(&b, "Ports: %s\n", strings.Join(c.Ports, ", "))
	}
	if len(c.Networks) > 0 {
		fmt.Fprintf(&b, "Networks: %s\n", strings.Join(c.Networks, ", "))
	}
	if env := containerInfoEnvView(c); env != "" {
		fmt.Fprintf(&b, "\n%s\n", env)
	}
	return b.String()
}

// containerInfoEnvView formats environment variables based on the container's image.
func containerInfoEnvView(c container) string {
	switch img := c.Image; {
	case strings.HasPrefix(img, "postgres"):
		return containerInfoEnvPostgresView(c.Env)
	default:
		return containerInfoEnvDefulatView(c.Env)
	}
}

// containerInfoEnvDefulatView provides a default rendering of environment variables.
func containerInfoEnvDefulatView(env map[string]string) string {
	var b strings.Builder
	for k, v := range env {
		fmt.Fprintf(&b, "%s: %s\n", k, v)
	}
	return b.String()
}

// containerInfoEnvPostgresView provides a specialized rendering for Postgres environment variables.
func containerInfoEnvPostgresView(env map[string]string) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Version: %s (%s)\n", env["PG_MAJOR"], env["PG_VERSION"])
	fmt.Fprintf(&b, "Database: %s\n", env["POSTGRES_DB"])
	fmt.Fprintf(&b, "User: %s\n", env["POSTGRES_USER"])
	fmt.Fprintf(&b, "Password: %s", env["POSTGRES_PASSWORD"])
	return b.String()
}

func (m ContainersModel) helpView() string {
	return lipgloss.
		NewStyle().
		Margin(1, 1, 0).
		Render(m.help.View(m.keys))
}

// container represents a Docker container with relevant details.
type container struct {
	ID       string
	Name     string
	Image    string
	State    string
	Status   string
	Ports    []string
	Created  time.Time
	Networks []string
	Mounts   []string
	Command  string
	Env      map[string]string
}

// containers retrieves the list of Docker containers and formats them as containerItem slices.
func containers() []container {
	all, err := dockerx.ContainerList(context.TODO())
	if err != nil {
		return []container{}
	}
	var containers []container
	for _, c := range all {
		ports := make([]string, len(c.Ports))
		for i, p := range c.Ports {
			ports[i] = fmt.Sprintf("%d:%d/%s", p.PublicPort, p.PrivatePort, p.Type)
		}
		containers = append(containers, container{
			ID:      c.ID,
			Name:    strings.TrimPrefix(c.Names[0], "/"),
			Image:   c.Image,
			State:   string(c.State),
			Status:  c.Status,
			Ports:   ports,
			Created: time.Unix(c.Created, 0),
			Command: c.Command,
			Env:     c.Env,
		})
	}
	return containers
}

// FilterValue returns the value used for filtering the list.
func (c container) FilterValue() string {
	return c.Name
}

// Title returns the title of the list item, including a status indicator.
func (c container) Title() string {
	status := "○"
	if c.State == "running" {
		status = "●"
	}
	return fmt.Sprintf("%s %s", status, c.Name)
}

// Description returns a brief description of the container item.
func (c container) Description() string {
	return fmt.Sprintf("%s • %s", c.Image, c.Status)
}

// tickMsg is a message sent at regular intervals to trigger updates.
type tickMsg struct{}

// tickCmd returns a command that sends a tickMsg after the specified period.
func tickCmd(period time.Duration) tea.Cmd {
	return tea.Tick(period, func(time.Time) tea.Msg {
		return tickMsg{}
	})
}

// startStopMsg represents the result of a start/stop container operation.
type startStopMsg struct {
	err error
}

// containerKeyMap are the default keybindings for the app.
type containerKeyMap struct {
	Create    key.Binding
	Delete    key.Binding
	StartStop key.Binding
	Quit      key.Binding
}

// newContainerKeyMap returns a new DefaultKeys.
func newContainerKeyMap() containerKeyMap {
	return containerKeyMap{
		Create:    key.NewBinding(key.WithKeys("c"), key.WithHelp("c", "create")),
		Delete:    key.NewBinding(key.WithKeys("d"), key.WithHelp("d", "delete")),
		StartStop: key.NewBinding(key.WithKeys("s"), key.WithHelp("s", "start/stop")),
		Quit:      key.NewBinding(key.WithKeys("esc"), key.WithHelp("esc", "quit")),
	}
}

// ShortHelp returns keybindings to be shown in the mini help view.
func (k containerKeyMap) ShortHelp() []key.Binding {
	return []key.Binding{
		k.Create, k.Delete, k.StartStop, k.Quit,
	}
}

// FullHelp returns keybindings for the expanded help view.
func (k containerKeyMap) FullHelp() [][]key.Binding {
	return [][]key.Binding{
		{k.Create, k.Delete, k.StartStop, k.Quit},
	}
}
