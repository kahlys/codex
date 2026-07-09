// Package app provides the core application logic for managing users in the Adminer application.
package app

import (
	"context"
	"errors"
	"log/slog"

	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
)

//go:generate go tool mockgen -destination=mock_test.go -package=app . UserStore

// UserStore defines the interface for user-related database operations in the Adminer application.
type UserStore interface {
	CreateUser(ctx context.Context, username, password string) (User, error)
	Users(ctx context.Context) ([]User, error)
	User(ctx context.Context, id int) (User, error)
	UpdateUser(ctx context.Context, id int, u User) (User, error)
}

var ErrNotFound = errors.New("not found")

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

// User infformation
type User struct {
	ID       int64
	Username string
	Password string
}

func toOpenAPIUser(user User) openapi.User {
	return openapi.User{
		Id:       int(user.ID),
		Username: user.Username,
		Password: user.Password,
	}
}

func toOpenAPIUsers(users []User) []openapi.User {
	res := make([]openapi.User, len(users))
	for i, user := range users {
		res[i] = toOpenAPIUser(user)
	}
	return res
}

// Users retrieves a list of all users from the UserStore and returns them.
func (s *Server) Users(ctx context.Context, _ openapi.UsersRequestObject) (openapi.UsersResponseObject, error) {
	s.logger.Info("GettingUsers")

	users, err := s.userStore.Users(ctx)
	if err != nil {
		return openapi.Users500Response{}, err
	}

	return openapi.Users200JSONResponse(toOpenAPIUsers(users)), nil
}

// User retrieves a user by their ID and returns the user information.
func (s *Server) User(ctx context.Context, req openapi.UserRequestObject) (openapi.UserResponseObject, error) {
	s.logger.Info("GettingUserByID", "id", req.Id)

	user, err := s.userStore.User(ctx, req.Id)
	if err != nil {
		if errors.Is(err, ErrNotFound) {
			return openapi.User404Response{}, nil
		}
		return openapi.User500Response{}, err
	}

	return openapi.User200JSONResponse(toOpenAPIUser(user)), nil
}

// CreateUser creates a new user with the given username and password, and returns the created user.
func (s *Server) CreateUser(ctx context.Context, req openapi.CreateUserRequestObject) (openapi.CreateUserResponseObject, error) {
	s.logger.Info("CreatingUser", "username", req.Body.Username)

	user, err := s.userStore.CreateUser(ctx, req.Body.Username, req.Body.Password)
	if err != nil {
		return openapi.CreateUser500Response{}, err
	}

	return openapi.CreateUser201JSONResponse(toOpenAPIUser(user)), nil
}

// UpdateUser updates an existing user's information based on the provided ID and request body, and returns the updated user.
func (s *Server) UpdateUser(ctx context.Context, req openapi.UpdateUserRequestObject) (openapi.UpdateUserResponseObject, error) {
	s.logger.Info("UpdatingUser", "id", req.Id)

	user, err := s.userStore.UpdateUser(
		ctx,
		req.Id,
		User{
			Username: req.Body.Username,
			Password: req.Body.Password,
		},
	)
	if err != nil {
		if errors.Is(err, ErrNotFound) {
			return openapi.UpdateUser404Response{}, nil
		}
		return openapi.UpdateUser500Response{}, err
	}

	return openapi.UpdateUser200JSONResponse(toOpenAPIUser(user)), nil
}
