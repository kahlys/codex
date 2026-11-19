package e2e

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"gopkg.in/yaml.v3"

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

func TestFileUploadAndDownload(t *testing.T) {
	t.Parallel()

	type data struct {
		Name    string `yaml:"name"`
		Version int    `yaml:"version"`
		Data    map[string]string
	}

	uploadedFile := data{
		Name:    "name",
		Version: 1,
		Data: map[string]string{
			"key1": "value1",
			"key2": "value2",
		},
	}

	server := httptest.NewServer(app.NewServer(newTestStore(t)).Handler())
	defer server.Close()

	yamlContent, err := yaml.Marshal(uploadedFile)
	require.NoError(t, err)

	t.Log("Uploading file")
	resp := httpPostFile(t, server.URL+"/files/upload", "file", "test.yaml", yamlContent)
	require.Equal(t, http.StatusCreated, resp.StatusCode)
	uploadResp := decodeJSON[openapi.UploadFile201JSONResponse](t, resp)
	require.NotNil(t, uploadResp.FileId)
	require.NotNil(t, uploadResp.Filename)
	assert.Equal(t, "test.yaml", *uploadResp.Filename)

	t.Log("Downloading file")
	resp = httpGet(t, server.URL+"/files/"+*uploadResp.FileId)
	require.Equal(t, http.StatusOK, resp.StatusCode)

	downloadedContent, err := io.ReadAll(resp.Body)
	require.NoError(t, err)

	var downloadedFile data
	require.NoError(t, yaml.Unmarshal(downloadedContent, &downloadedFile))
	assert.Equal(t, uploadedFile, downloadedFile)

	t.Log("Downloading non-existent file")
	resp = httpGet(t, server.URL+"/files/nonexistent-id")
	require.Equal(t, http.StatusNotFound, resp.StatusCode)
}
