package app

import (
	"context"
	"errors"

	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
)

var ErrNotFound = errors.New("not found")

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
	s.log(ctx).Info("GettingUsers")

	users, err := s.userStore.Users(ctx)
	if err != nil {
		return openapi.Users500Response{}, err
	}

	return openapi.Users200JSONResponse(toOpenAPIUsers(users)), nil
}

// User retrieves a user by their ID and returns the user information.
func (s *Server) User(ctx context.Context, req openapi.UserRequestObject) (openapi.UserResponseObject, error) {
	s.log(ctx).Info("GettingUserByID", "id", req.Id)

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
	s.log(ctx).Info("CreatingUser", "username", req.Body.Username)

	user, err := s.userStore.CreateUser(ctx, req.Body.Username, req.Body.Password)
	if err != nil {
		return openapi.CreateUser500Response{}, err
	}

	return openapi.CreateUser201JSONResponse(toOpenAPIUser(user)), nil
}

// UpdateUser updates an existing user's information based on the provided ID and request body, and returns the updated user.
func (s *Server) UpdateUser(ctx context.Context, req openapi.UpdateUserRequestObject) (openapi.UpdateUserResponseObject, error) {
	s.log(ctx).Info("UpdatingUser", "id", req.Id)

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
