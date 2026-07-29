package main

import (
	"bytes"
	"context"
	"encoding/json"
	"io/fs"
	"log/slog"
	"net/http"
	"os"
	"text/template"
	"time"

	"github.com/kahlys/codex/template/adminer/internal/gui/assets"
)

// User represents a user from the API
type User struct {
	ID       int    `json:"id"`
	Username string `json:"username"`
	Password string `json:"password"`
}

// The application struct holds the dependencies needed for our handlers,
// including a htmlRenderer type.
type application struct {
	logger     *slog.Logger
	html       *htmlRenderer
	apiURL     string
	httpClient *http.Client
}

func main() {
	logger := slog.New(slog.NewTextHandler(os.Stdout, nil))

	// Initialize a new htmlRenderer, parsing the base template and all partial
	// templates from assets/html into the shared template set.
	htmlRenderer, err := newHTMLRenderer(assets.HTMLFiles, "base.tmpl", "partials/*.tmpl")
	if err != nil {
		logger.Error(err.Error())
		os.Exit(1)
	}

	// Include the htmlRenderer in the application struct.
	app := &application{
		logger:     logger,
		html:       htmlRenderer,
		apiURL:     "http://localhost:8080",
		httpClient: &http.Client{Timeout: 5 * time.Second},
	}

	// Create a file server that serves the files from assets/static.
	fileserver := http.FileServerFS(assets.StaticFiles)

	// Register the application routes.
	mux := http.NewServeMux()
	mux.Handle("GET /static/", http.StripPrefix("/static", fileserver))
	mux.HandleFunc("GET /{$}", app.home)
	mux.HandleFunc("GET /gopher", app.gopher)
	mux.HandleFunc("GET /users", app.usersPage)
	mux.HandleFunc("GET /users/table", app.usersTable)

	// Start the HTTP server.
	logger.Info("starting server", "address", "http://localhost:5051")
	err = http.ListenAndServe(":5051", mux)
	if err != nil {
		logger.Error(err.Error())
		os.Exit(1)
	}
}

// --- render

type htmlRenderer struct {
	templateFS      fs.FS
	sharedTemplates *template.Template
}

// The newHTMLRenderer function creates a new htmlRenderer containing a shared
// set of parsed templates with support for any custom template functions.
func newHTMLRenderer(templateFS fs.FS, sharedTemplateFiles ...string) (*htmlRenderer, error) {
	funcs := template.FuncMap{
		"now": time.Now,
		// Other custom template functions go here...
	}

	sharedTemplates, err := template.New("").Funcs(funcs).ParseFS(templateFS, sharedTemplateFiles...)
	if err != nil {
		return nil, err
	}

	r := &htmlRenderer{
		templateFS:      templateFS,
		sharedTemplates: sharedTemplates,
	}

	return r, nil
}

// The render method clones the shared template set, optionally parses additional
// templates, executes the named template with the supplied data, and writes the
// response.
func (h *htmlRenderer) render(w http.ResponseWriter, status int, data any, templateName string, additionalTemplateFiles ...string) error {
	ts, err := h.sharedTemplates.Clone()
	if err != nil {
		return err
	}

	if len(additionalTemplateFiles) > 0 {
		ts, err = ts.ParseFS(h.templateFS, additionalTemplateFiles...)
		if err != nil {
			return err
		}
	}

	buf := new(bytes.Buffer)

	err = ts.ExecuteTemplate(buf, templateName, data)
	if err != nil {
		return err
	}

	w.WriteHeader(status)
	buf.WriteTo(w)

	return nil
}

// --- handlers

func (app *application) home(w http.ResponseWriter, r *http.Request) {
	err := app.html.render(w, 200, nil, "base", "pages/home.tmpl")
	if err != nil {
		app.logger.Error(err.Error())
		http.Error(w, http.StatusText(500), 500)
	}
}

func (app *application) gopher(w http.ResponseWriter, r *http.Request) {
	width := 100
	err := app.html.render(w, http.StatusOK, width, "partial:image:gopher")
	if err != nil {
		app.logger.Error(err.Error())
		http.Error(w, http.StatusText(500), 500)
	}
}

func (app *application) usersPage(w http.ResponseWriter, r *http.Request) {
	// Render the users page with HTMX container
	err := app.html.render(w, 200, nil, "base", "pages/users.tmpl")
	if err != nil {
		app.logger.Error(err.Error())
		http.Error(w, http.StatusText(500), 500)
	}
}

func (app *application) usersTable(w http.ResponseWriter, r *http.Request) {
	time.Sleep(2 * time.Second) // Simulate a delay for demonstration purposes

	// Fetch users from the API
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	req, err := http.NewRequestWithContext(ctx, "GET", app.apiURL+"/users", nil)
	if err != nil {
		app.logger.Error("failed to create request", "error", err)
		http.Error(w, http.StatusText(500), 500)
		return
	}

	resp, err := app.httpClient.Do(req)
	if err != nil {
		app.logger.Error("failed to fetch users", "error", err)
		http.Error(w, http.StatusText(500), 500)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		app.logger.Error("unexpected status code", "status", resp.StatusCode)
		http.Error(w, http.StatusText(500), 500)
		return
	}

	var users []User
	err = json.NewDecoder(resp.Body).Decode(&users)
	if err != nil {
		app.logger.Error("failed to decode users", "error", err)
		http.Error(w, http.StatusText(500), 500)
		return
	}

	// Render just the table HTML (for HTMX)
	w.Header().Set("Content-Type", "text/html")
	err = app.html.render(w, 200, users, "users:table", "pages/users.tmpl")
	if err != nil {
		app.logger.Error(err.Error())
		http.Error(w, http.StatusText(500), 500)
	}
}
