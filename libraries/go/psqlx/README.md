# psqlx

A wrapper around the [pgx](https://github.com/jackc/pgx) PostgreSQL driver that returns specific error for connection issues.

## Usage

Run PostgreSQL in Docker for testing:

```bash
docker run -d --name postgres-test -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres:15
```

Example:

```go
package main

import (
	"database/sql"
	"errors"
	"fmt"
	"log"

	"github.com/kahlys/codex/go/pkg/psqlx"
)

// ConfigDB wraps a database connection
type ConfigDB struct {
	DB *sql.DB
}

// NewConfigDB creates a new ConfigDB instance with the provided datasource
func NewConfigDB(datasource string) *ConfigDB {
	return &ConfigDB{
		DB: psqlx.Open(datasource),
	}
}

// SelectOne performs a simple SELECT 1 query to test the connection
func (c *ConfigDB) SelectOne() error {
	var result int
	err := c.DB.QueryRow("SELECT 1").Scan(&result)
	if err != nil {
		return err
	}
	if result != 1 {
		return fmt.Errorf("unexpected result: %d", result)
	}
	return nil
}

func main() {
	datasource := "postgres://postgres:postgres@localhost:5432/postgres?sslmode=disable"

	config := NewConfigDB(datasource)

	if err := config.SelectOne(); err != nil {
		if errors.Is(err, psqlx.ErrDatabaseUnreachable) {
			log.Println("Database is unreachable")
		} else {
			log.Println(err)
		}
	}
}

```
