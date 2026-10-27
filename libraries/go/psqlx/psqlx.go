// Package psqlx provides helpers to open PostgreSQL connections with pgx.
package psqlx

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"errors"
	"fmt"

	"github.com/jackc/pgx/v5/stdlib"
)

// ErrDatabaseUnreachable is returned when the database cannot be reached
var ErrDatabaseUnreachable = errors.New("database is unreachable")

// Open opens a new database connection pool using the provided datasource.
func Open(datasource string) *sql.DB {
	return sql.OpenDB(connectorPgx{dsn: datasource, driver: driverPgx{}})
}

type connectorPgx struct {
	dsn    string
	driver driver.Driver
}

func (t connectorPgx) Connect(_ context.Context) (driver.Conn, error) {
	return t.driver.Open(t.dsn)
}

func (t connectorPgx) Driver() driver.Driver {
	return t.driver
}

type driverPgx struct{}

func (d driverPgx) Open(name string) (driver.Conn, error) {
	conn, err := stdlib.GetDefaultDriver().Open(name)
	if err != nil {
		return nil, fmt.Errorf("%w: %w", ErrDatabaseUnreachable, err)
	}
	return conn, nil
}
