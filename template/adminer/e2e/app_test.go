package e2e

import (
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
	"github.com/kahlys/codex/template/adminer/internal/app"
)

func TestUser(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(app.NewServer(newTestStore(t)).Handler())
	defer server.Close()

	t.Log("Creating user alice")
	resp := httpPost(t, server.URL+"/users", openapi.CreateUserJSONRequestBody{Username: "alice", Password: "secret"})
	require.Equal(t, http.StatusCreated, resp.StatusCode)
	alice := decodeJSON[openapi.User](t, resp)

	t.Log("Creating user bob")
	resp = httpPost(t, server.URL+"/users", openapi.CreateUserJSONRequestBody{Username: "bob", Password: "secret"})
	require.Equal(t, http.StatusCreated, resp.StatusCode)
	bob := decodeJSON[openapi.User](t, resp)

	t.Log("Listing users")
	resp = httpGet(t, server.URL+"/users")
	require.Equal(t, http.StatusOK, resp.StatusCode)
	users := decodeJSON[[]openapi.User](t, resp)
	assert.ElementsMatch(t, []openapi.User{alice, bob}, users)

	t.Log("Retrieving user alice")
	resp = httpGet(t, server.URL+"/users/"+strconv.Itoa(alice.Id))
	require.Equal(t, http.StatusOK, resp.StatusCode)
	gotAlice := decodeJSON[openapi.User](t, resp)
	assert.Equal(t, alice, gotAlice)

	t.Log("Retrieving unknown user")
	resp = httpGet(t, server.URL+"/users/999999")
	require.Equal(t, http.StatusNotFound, resp.StatusCode)
}
