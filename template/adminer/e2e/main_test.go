package e2e

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/jackc/pgx/v5/stdlib"
	"github.com/ory/dockertest/v3"
	"github.com/ory/dockertest/v3/docker"
	"github.com/stretchr/testify/require"

	"github.com/kahlys/codex/template/adminer/internal/store"
	"github.com/kahlys/codex/template/adminer/internal/store/migration"
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

	// Wrap pgxpool as *sql.DB for migrations and store
	sqlDB := stdlib.OpenDBFromPool(db)
	defer func() {
		_ = sqlDB.Close()
	}()

	if _, err := migration.Migrate(sqlDB, "file://../sql/migrations"); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to run migrations: %v\n", err)
		os.Exit(1)
	}

	code := m.Run()

	if err := dockerPool.Purge(resource); err != nil {
		fmt.Fprintf(os.Stderr, "Could not purge resource: %s\n", err)
	}

	os.Exit(code)
}

func newTestStore(t *testing.T) *store.UserStore {
	t.Helper()
	sqlDB := stdlib.OpenDBFromPool(adminDB)
	return store.NewUserStore(sqlDB)
}

func httpPost(t *testing.T, url string, body any) *http.Response {
	t.Helper()
	b, err := json.Marshal(body)
	require.NoError(t, err)
	resp, err := http.Post(url, "application/json", bytes.NewReader(b))
	require.NoError(t, err)
	t.Cleanup(func() { resp.Body.Close() })
	return resp
}

func httpGet(t *testing.T, url string) *http.Response {
	t.Helper()
	resp, err := http.Get(url)
	require.NoError(t, err)
	t.Cleanup(func() { resp.Body.Close() })
	return resp
}

func decodeJSON[T any](t *testing.T, resp *http.Response) T {
	t.Helper()
	var v T
	require.NoError(t, json.NewDecoder(resp.Body).Decode(&v))
	return v
}
