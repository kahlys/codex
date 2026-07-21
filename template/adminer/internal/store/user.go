// Package store provides implementations of the adminer.UserStore interface for managing user data in a PostgreSQL database.
package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	"github.com/kahlys/codex/template/adminer/internal/app"
)

// UserStore implements the adminer.UserStore interface using a PostgreSQL database.
type UserStore struct {
	db *sql.DB
}

var _ app.UserStore = (*UserStore)(nil)

// NewUserStore creates a new UserStore with the given database connection.
func NewUserStore(db *sql.DB) *UserStore {
	return &UserStore{db: db}
}

// CreateUser creates a new user in the database and returns the created user.
func (s *UserStore) CreateUser(ctx context.Context, username, password string) (app.User, error) {
	var user app.User

	if err := s.db.QueryRowContext(
		ctx,
		`INSERT INTO users (username, password)
		 VALUES ($1, $2)
		 RETURNING id, username, password`,
		username,
		password,
	).Scan(
		&user.ID,
		&user.Username,
		&user.Password,
	); err != nil {
		return app.User{}, err
	}

	return user, nil
}

// Users retrieves all users from the database.
func (s *UserStore) Users(ctx context.Context) ([]app.User, error) {
	rows, err := s.db.QueryContext(
		ctx,
		`SELECT id, username, password FROM users`,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []app.User
	for rows.Next() {
		var user app.User
		if err := rows.Scan(
			&user.ID,
			&user.Username,
			&user.Password,
		); err != nil {
			return nil, err
		}
		users = append(users, user)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return users, nil
}

// User retrieves a user by their ID from the database.
func (s *UserStore) User(ctx context.Context, id int) (app.User, error) {
	var user app.User

	if err := s.db.QueryRowContext(
		ctx,
		`SELECT id, username, password
		 FROM users
		 WHERE id = $1`,
		id,
	).Scan(
		&user.ID,
		&user.Username,
		&user.Password,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return app.User{}, fmt.Errorf("fetch user %v from database : %w", id, app.ErrNotFound)
		}
		return app.User{}, err
	}

	return user, nil
}

func (s *UserStore) DeleteUser(ctx context.Context, id int) error {
	result, err := s.db.ExecContext(
		ctx,
		`DELETE FROM users WHERE id = $1`,
		id,
	)
	if err != nil {
		return err
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return err
	}

	if rowsAffected == 0 {
		return fmt.Errorf("delete user %v from database : %w", id, app.ErrNotFound)
	}

	return nil
}

func (s *UserStore) UpdateUser(ctx context.Context, id int, u app.User) (app.User, error) {
	var user app.User

	if err := s.db.QueryRowContext(
		ctx,
		`UPDATE users
		 SET username = $1, password = $2
		 WHERE id = $3
		 RETURNING id, username, password`,
		u.Username,
		u.Password,
		id,
	).Scan(
		&user.ID,
		&user.Username,
		&user.Password,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return app.User{}, fmt.Errorf("update user %v in database : %w", id, app.ErrNotFound)
		}
		return app.User{}, err
	}

	return user, nil
}
