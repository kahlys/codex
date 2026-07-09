package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"

	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/kahlys/codex/template/adminer/internal/store/migration"
)

func main() {
	if err := run(); err != nil {
		os.Exit(1)
	}
}

func run() error {
	dbUser := flag.String("db-user", "postgres", "Postgres user")
	dbPassword := flag.String("db-password", "postgres", "Postgres password")
	dbHost := flag.String("db-host", "database", "Postgres host")
	dbPort := flag.String("db-port", "5432", "Postgres port")
	dbName := flag.String("db-name", "postgres", "Postgres database name")
	dbSSLMode := flag.String("db-sslmode", "disable", "Postgres SSL mode")
	flag.Parse()

	db, err := pgxpool.New(
		context.Background(),
		fmt.Sprintf(
			"postgres://%s:%s@%s:%s/%s?sslmode=%s",
			*dbUser,
			*dbPassword,
			*dbHost,
			*dbPort,
			*dbName,
			*dbSSLMode,
		),
	)
	if err != nil {
		slog.Error("InitFailed", "error", err, "step", "database connection")
		return err
	}

	if _, err := migration.Migrate(db, "file://sql/migrations"); err != nil {
		slog.Error("InitFailed", "error", err, "step", "database migrations")
		return err
	}

	return nil
}
