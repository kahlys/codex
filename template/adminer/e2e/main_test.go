//go:build e2e

package e2e_test

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/ory/dockertest/v3"
	"github.com/ory/dockertest/v3/docker"

	"github.com/kahlys/codex/template/adminer/adminer"
	"github.com/kahlys/codex/template/adminer/adminer/store"
	"github.com/kahlys/codex/template/adminer/adminer/store/migration"
)

var (
	dbPassword = "postgres"
	dbUser     = "postgres"
	dbName     = "postgres"
	adminDB    *pgxpool.Pool
)

func TestMain(m *testing.M) {
	dockerPool, err := dockertest.NewPool("")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}

	if err = dockerPool.Client.Ping(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}

	resource, err := dockerPool.RunWithOptions(&dockertest.RunOptions{
		Repository: "postgres",
		Tag:        "11",
		Env: []string{
			fmt.Sprintf("POSTGRES_PASSWORD=%s", dbPassword),
			fmt.Sprintf("POSTGRES_USER=%s", dbUser),
			fmt.Sprintf("POSTGRES_DB=%s", dbName),
			"listen_addresses = '*'",
		},
	}, func(config *docker.HostConfig) {
		config.AutoRemove = true
		config.RestartPolicy = docker.RestartPolicy{Name: "no"}
	})
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	_ = resource.Expire(120)

	var db *pgxpool.Pool

	dockerPool.MaxWait = 10 * time.Second
	if err = dockerPool.Retry(func() error {
		db, err = pgxpool.New(
			context.Background(),
			fmt.Sprintf(
				"postgres://%s:%s@%s/%s?sslmode=disable",
				dbUser,
				dbPassword,
				resource.GetHostPort("5432/tcp"),
				dbName,
			),
		)
		if err != nil {
			return err
		}
		return db.Ping(context.Background())
	}); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}

	adminDB = db

	if _, err := migration.Migrate(db, "file://../sql/migrations"); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to run migrations: %v\n", err)
		os.Exit(1)
	}

	code := m.Run()

	if err := dockerPool.Purge(resource); err != nil {
		fmt.Fprintf(os.Stderr, "Could not purge resource: %s\n", err)
	}

	os.Exit(code)
}

func newTestServer(t *testing.T) (*adminer.Server, func()) {
	t.Helper()

	tx, err := adminDB.Begin(t.Context())
	if err != nil {
		t.Fatalf("begin transaction: %v", err)
	}

	server := adminer.NewServer(store.NewUserStore(adminDB))
	return server, func() {
		tx.Rollback(t.Context())
	}
}
