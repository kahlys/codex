package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net/http"
	"os"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/jackc/pgx/v5/stdlib"

	"github.com/kahlys/codex/template/adminer/internal/app"
	"github.com/kahlys/codex/template/adminer/internal/store"
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
	dbHost := flag.String("db-host", "localhost", "Postgres host")
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

	// Wrap pgxpool as *sql.DB for store
	sqlDB := stdlib.OpenDBFromPool(db)

	slog.Info("Init", "step", "database migration")
	if _, err := migration.Migrate(sqlDB, "file://sql/migrations"); err != nil {
		slog.Error("InitFailed", "error", err, "step", "database migrations")
		return err
	}

	slog.Info("Init", "step", "starting server")
	myapp := app.NewServer(store.NewUserStore(sqlDB))

	httpServer := &http.Server{
		Addr:    "0.0.0.0:8080",
		Handler: myapp.Handler(),
	}

	slog.Info("ServerStart", "addr", httpServer.Addr)
	if err := httpServer.ListenAndServe(); err != nil {
		slog.Error("ServerStopped", "error", err)
		return err
	}

	// Close the pgxpool after server shuts down
	db.Close()

	return nil
}
