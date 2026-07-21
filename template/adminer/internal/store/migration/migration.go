// Package migration provides functionality to apply database migrations using the golang-migrate library.
package migration

import (
	"database/sql"
	"fmt"

	"github.com/golang-migrate/migrate/v4"
	"github.com/golang-migrate/migrate/v4/database/postgres"
	_ "github.com/golang-migrate/migrate/v4/source/file" // file source for migration
)

// Migrate applies database migrations from the specified path using the provided database connection.
// An optional schemaName can be provided to target a specific schema (defaults to "public").
func Migrate(db *sql.DB, migrationsPath string, schemaName ...string) (version uint, err error) {
	cfg := &postgres.Config{}
	if len(schemaName) > 0 && schemaName[0] != "" {
		cfg.SchemaName = schemaName[0]
	}

	driver, err := postgres.WithInstance(db, cfg)
	if err != nil {
		return 0, fmt.Errorf("failed to create migration driver: %w", err)
	}

	m, err := migrate.NewWithDatabaseInstance(
		migrationsPath,
		"postgres",
		driver,
	)
	if err != nil {
		return 0, fmt.Errorf("failed to create migrator: %w", err)
	}

	if err := m.Up(); err != nil && err != migrate.ErrNoChange {
		return 0, fmt.Errorf("failed to run migrations: %w", err)
	}

	version, _, err = m.Version()
	if err != nil && err != migrate.ErrNilVersion {
		return 0, fmt.Errorf("failed to get migration version: %w", err)
	}

	return version, nil
}
