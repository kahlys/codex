package model

import (
	"context"
	"fmt"

	tea "charm.land/bubbletea/v2"
	"charm.land/huh/v2"
	"charm.land/lipgloss/v2"

	"github.com/kahlys/codex/go/cmd/bazaar/internal/dockerx"
	"github.com/kahlys/codex/go/cmd/bazaar/internal/teax"
)

type step int

const (
	stepImage step = iota
	stepData
)

const (
	imgPostgres string = "PostgreSQL"
	imgMySQL    string = "MySQL"
	imgRedis    string = "Redis"
)

// CreateModel is a Bubble Tea model for creating new Docker containers.
type CreateModel struct {
	step      step
	formImage *huh.Form
	formData  *huh.Form
	width     int
	height    int
}

// NewCreateModel initializes and returns a new CreateModel instance.
func NewCreateModel() CreateModel {
	return CreateModel{
		step:      stepImage,
		formImage: newFormImage(),
		formData:  nil,
	}
}

// newFormImage creates a form for selecting the image of container to create.
func newFormImage() *huh.Form {
	return huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Key("image").
				Title("Choose an image").
				Validate(func(v string) error {
					if v == imgMySQL || v == imgRedis {
						return fmt.Errorf("not implemented yet")
					}
					return nil
				}).
				Options(huh.NewOptions(imgPostgres, imgMySQL, imgRedis)...),
		),
	)
}

// newFormData creates a form for the selected image.
func newFormData(image string) *huh.Form {
	switch image {
	case imgPostgres:
		return newFormDataPSQL()
	default:
		panic("unsupported: " + image)
	}
}

// newFormDataPSQL creates a form for PostgreSQL container configuration.
func newFormDataPSQL() *huh.Form {
	return huh.NewForm(
		huh.NewGroup(
			huh.NewInput().
				Key("name").
				Title("Name").
				Validate(validName),
			huh.NewInput().
				Key("version").
				Title("Version (default: latest)"),
			huh.NewInput().
				Key("port").
				Title("Port").
				Validate(validPort),
		),
	).WithWidth(40)
}

// Init is the initial command for the CreateModel.
func (m CreateModel) Init() tea.Cmd {
	return m.formImage.Init()
}

// Update handles incoming messages and updates the model state accordingly.
func (m CreateModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
			return m, tea.Quit
		case "esc":
			if m.step == stepData {
				m.formImage = newFormImage()
				m.step = stepImage
				return m, m.formImage.Init()
			}
			return teax.SwitchModel(NewContainersModel())
		}
	case tea.WindowSizeMsg:
		titleHeight := lipgloss.Height(titleView())
		m.width = msg.Width
		m.height = msg.Height - titleHeight
		return m, nil
	}

	switch m.step {
	case stepImage:
		formModel, cmd := m.formImage.Update(msg)
		if form, ok := formModel.(*huh.Form); ok {
			m.formImage = form
			if form.State == huh.StateCompleted {
				m.formData = newFormData(m.selectedImage())
				m.step = stepData
				return m, m.formData.Init()
			}
		}
		return m, cmd
	case stepData:
		formModel, cmd := m.formData.Update(msg)
		if form, ok := formModel.(*huh.Form); ok {
			m.formData = form
			if form.State == huh.StateCompleted {
				m.runContainer()
				return teax.SwitchModel(NewContainersModel())
			}
		}
		return m, cmd
	}
	return m, nil
}

// runContainer creates and starts a new container based on the selected image and form data.
func (m CreateModel) runContainer() {
	switch image := m.selectedImage(); image {
	case imgPostgres:
		m.runContainerPSQL()
	default:
		panic("unsupported: " + image)
	}
}

// runContainerPSQL creates and starts a new PostgreSQL container based on the form data.
func (m CreateModel) runContainerPSQL() {
	version := "latest"
	if v := m.formData.GetString("version"); v != "" {
		version = v
	}

	_ = dockerx.ContainerStartNew(
		context.Background(),
		dockerx.Config{
			Image:         "postgres",
			Tag:           version,
			Name:          m.formData.GetString("name"),
			HostIP:        "0.0.0.0",
			HostPort:      m.formData.GetString("port"),
			ContainerPort: "5432",
			Env: []string{
				"POSTGRES_USER=postgres",
				"POSTGRES_PASSWORD=postgres",
				"POSTGRES_DB=postgres",
			},
		})
}

func (m CreateModel) selectedImage() string {
	return m.formImage.GetString("image")
}

// View renders the UI based on the current step.
func (m CreateModel) View() tea.View {
	var formView string
	switch m.step {
	case stepImage:
		formView = m.formImage.View()
	case stepData:
		formView = m.formData.View()
	default:
		panic("unsupported step")
	}

	borderH, borderV := borderSize()

	return teax.View(
		lipgloss.JoinVertical(
			lipgloss.Left,
			titleView(),
			Border.
				Width(m.width-borderH).
				Height(m.height-borderV).
				Render(formView),
		),
	)
}

// validName checks if the provided container name is valid and not already in use.
func validName(s string) error {
	if s == "" {
		return fmt.Errorf("name is required")
	}

	if err := dockerx.ContainerNameAvailable(context.TODO(), s); err != nil {
		return err
	}

	return nil
}

// validPort checks if the provided string is a valid port number (1-65535).
func validPort(s string) error {
	if s == "" {
		return fmt.Errorf("port is required")
	}
	var port int
	if _, err := fmt.Sscanf(s, "%d", &port); err != nil || port < 1 || port > 65535 {
		return fmt.Errorf("invalid port number")
	}
	return nil
}
