// Package store provides implementations of the adminer.UserStore interface for managing user data in a PostgreSQL database.
package store

import (
	"context"

	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/kahlys/codex/template/adminer/adminer"
)

// UserStore implements the adminer.UserStore interface using a PostgreSQL database.
type UserStore struct {
	db *pgxpool.Pool
}

var _ adminer.UserStore = (*UserStore)(nil)

// NewUserStore creates a new UserStore with the given database connection pool.
func NewUserStore(db *pgxpool.Pool) *UserStore {
	return &UserStore{db: db}
}

// CreateUser creates a new user in the database and returns the created user.
func (s *UserStore) CreateUser(ctx context.Context, username, password string) (adminer.User, error) {
	var user adminer.User

	if err := s.db.QueryRow(
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
		return adminer.User{}, err
	}

	return user, nil
}

// Users retrieves all users from the database.
func (s *UserStore) Users(ctx context.Context) ([]adminer.User, error) {
	rows, err := s.db.Query(
		ctx,
		`SELECT id, username, password FROM users`,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []adminer.User
	for rows.Next() {
		var user adminer.User
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

// UserByUsername retrieves a user by their username from the database.
func (s *UserStore) UserByUsername(ctx context.Context, username string) (adminer.User, error) {
	var user adminer.User

	if err := s.db.QueryRow(
		ctx,
		`SELECT id, username, password
		 FROM users
		 WHERE username = $1`,
		username,
	).Scan(
		&user.ID,
		&user.Username,
		&user.Password,
	); err != nil {
		return adminer.User{}, err
	}

	return user, nil
}
