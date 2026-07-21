package app

import (
	"context"
	"log/slog"
	"net/http"

	"github.com/getkin/kin-openapi/openapi3filter"
	"github.com/gorilla/mux"
	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
	middleware "github.com/oapi-codegen/nethttp-middleware"
)

//go:generate go tool mockgen -destination=mock_test.go -package=app . UserStore

// UserStore defines the interface for user-related database operations in the Adminer application.
type UserStore interface {
	CreateUser(ctx context.Context, username, password string) (User, error)
	Users(ctx context.Context) ([]User, error)
	User(ctx context.Context, id int) (User, error)
	UpdateUser(ctx context.Context, id int, u User) (User, error)
}

// Server represents the main application server that handles user-related operations and interacts with the UserStore for database access.
type Server struct {
	userStore UserStore
	logger    *slog.Logger
}

var _ openapi.StrictServerInterface = (*Server)(nil)

// NewServer creates a new Server instance with the provided UserStore and optional configuration options.
func NewServer(userStore UserStore, opts ...ServerOption) *Server {
	s := &Server{
		userStore: userStore,
		logger:    slog.Default(),
	}
	for _, opt := range opts {
		opt(s)
	}
	return s
}

// Handler returns the fully configured HTTP handler
func (s *Server) Handler() http.Handler {
	specs, err := openapi.GetSpec()
	if err != nil {
		slog.Error("Failed to load OpenAPI specs", "error", err)
		panic(err)
	}

	router := mux.NewRouter()
	router.Use(
		s.mwLog,
		middleware.OapiRequestValidatorWithOptions(specs,
			&middleware.Options{
				Options: openapi3filter.Options{
					AuthenticationFunc: authFunc,
				},
			},
		),
	)

	handler := openapi.HandlerFromMux(
		openapi.NewStrictHandler(s, nil),
		router,
	)

	// Wrap with OpenTelemetry HTTP instrumentation
	// return otelhttp.NewHandler(handler, "http-server")

	return handler
}

// authFunc handles authentication for OpenAPI requests
func authFunc(ctx context.Context, input *openapi3filter.AuthenticationInput) error {
	req := input.RequestValidationInput.Request

	// Example: Get cookie if the security scheme uses cookies
	if input.SecurityScheme.Type == "apiKey" && input.SecurityScheme.In == "cookie" {
		cookie, err := req.Cookie(input.SecurityScheme.Name)
		if err != nil {
			slog.Warn("Cookie not found",
				"cookie_name", input.SecurityScheme.Name,
				"error", err,
			)
		} else {
			slog.Info("Cookie found",
				"cookie_name", input.SecurityScheme.Name,
				"cookie_value", cookie.Value,
			)
		}
	}

	// Log security information
	slog.Info("SecurityCheck",
		"scheme_name", input.SecuritySchemeName,
		"scheme_type", input.SecurityScheme.Type,
		"scheme_in", input.SecurityScheme.In,
		"path", input.RequestValidationInput.Route.Path,
		"method", req.Method,
	)

	// For now, just allow everything (in production, you'd validate here)
	return nil
}
