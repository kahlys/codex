// Package adminer provides the core application logic for managing users in the Adminer application.
package adminer

import (
	"context"
	"log/slog"
)

// UserStore defines the interface for user-related database operations in the Adminer application.
type UserStore interface {
	CreateUser(ctx context.Context, username, password string) (User, error)
	Users(ctx context.Context) ([]User, error)
	UserByUsername(ctx context.Context, username string) (User, error)
}

// Server represents the main application server that handles user-related operations and interacts with the UserStore for database access.
type Server struct {
	userStore UserStore
	logger    *slog.Logger
}

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

// User infformation
type User struct {
	ID       int64
	Username string
	Password string
}

// CreateUser creates a new user with the given username and password, and returns the created user.
func (s *Server) CreateUser(ctx context.Context, username, password string) (User, error) {
	s.logger.Info("CreatingUser", "username", username)
	return s.userStore.CreateUser(ctx, username, password)
}

// Users retrieves a list of all users from the UserStore and returns them.
func (s *Server) Users(ctx context.Context) ([]User, error) {
	s.logger.Info("GettingUsers")
	return s.userStore.Users(ctx)
}

// UserByUsername retrieves a user by their username and returns the user information.
func (s *Server) UserByUsername(ctx context.Context, username string) (User, error) {
	s.logger.Info("GettingUserByUsername", "username", username)
	return s.userStore.UserByUsername(ctx, username)
}
