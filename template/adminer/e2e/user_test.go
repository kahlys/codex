//go:build e2e

package e2e_test

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestUser(t *testing.T) {
	t.Parallel()

	server, cleanup := newTestServer(t)
	defer cleanup()

	t.Log("Creating user")
	alice, err := server.CreateUser(t.Context(), "alice", "secret")
	assert.NoError(t, err)
	_, err = server.CreateUser(t.Context(), "bob", "secret")
	assert.NoError(t, err)

	t.Log("Listing users")
	users, err := server.Users(t.Context())
	assert.NoError(t, err)
	assert.Len(t, users, 2)

	t.Log("Fetching user")
	fetched, err := server.UserByUsername(t.Context(), "alice")
	assert.NoError(t, err)
	assert.Equal(t, alice.ID, fetched.ID)
	assert.Equal(t, alice.Username, fetched.Username)
	assert.Equal(t, alice.Password, fetched.Password)

	t.Log("Fetching non-existing user")
	_, err = server.UserByUsername(t.Context(), "eve")
	assert.Error(t, err)

	t.Log("Test passed")
}
