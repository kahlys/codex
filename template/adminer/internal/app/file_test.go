package app

import (
	"bytes"
	"io"
	"mime/multipart"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
)

func TestServer_UploadFile(t *testing.T) {
	tests := map[string]struct {
		file             *multipart.Reader
		wantResponseType string
		wantFileStored   bool
	}{
		"ok": {
			file:             createTestFile(t, "file", "config.yml", "nested:\n  key1: value1\n  key2: value2\n"),
			wantResponseType: "UploadFile201JSONResponse",
			wantFileStored:   true,
		},
		"err_invalid_yml": {
			file:             newMultipartReader("config.yaml", "key1: \"value1\nkey2: value2\n"),
			wantResponseType: "UploadFile400Response",
			wantFileStored:   false,
		},
		"err_txt": {
			file:             createTestFile(t, "file", "config.txt", "sometext\n"),
			wantResponseType: "UploadFile400Response",
			wantFileStored:   false,
		},
		"err_nil": {
			file:             nil,
			wantResponseType: "UploadFile400Response",
			wantFileStored:   false,
		},
	}

	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			mockCtrl := NewMockUserStore(nil)
			server := NewServer(mockCtrl)

			resp, err := server.UploadFile(
				t.Context(),
				openapi.UploadFileRequestObject{
					Body: tt.file,
				},
			)
			require.NoError(t, err)

			// Verify response type
			switch tt.wantResponseType {
			case "UploadFile201JSONResponse":
				respJSON, ok := resp.(openapi.UploadFile201JSONResponse)
				assert.True(t, ok, "response should be UploadFile201JSONResponse")
				assert.NotNil(t, respJSON.FileId, "fileId should not be nil")
				assert.NotNil(t, respJSON.Filename, "filename should not be nil")
			case "UploadFile400Response":
				_, ok := resp.(openapi.UploadFile400Response)
				assert.True(t, ok, "response should be UploadFile400Response")
			}

			if tt.wantFileStored {
				assert.NotEmpty(t, server.fileStore, "file should be stored in fileStore")
				assert.Greater(t, len(server.fileStore), 0, "fileStore should contain at least one entry")
			} else {
				assert.Empty(t, server.fileStore, "fileStore should be empty when file is rejected")
			}
		})
	}
}

func TestServer_DownloadFile(t *testing.T) {
	tests := map[string]struct {
		fileID           string
		setupStore       bool
		wantResponseType string
		wantContent      string
	}{
		"ok": {
			fileID:           "test-file-id",
			setupStore:       true,
			wantResponseType: "DownloadFile200ApplicationoctetStreamResponse",
			wantContent:      "nested:\n    key1: value1\n    key2: value2\n",
		},
		"err_not_found": {
			fileID:           "nonexistent-id",
			setupStore:       false,
			wantResponseType: "DownloadFile404Response",
			wantContent:      "",
		},
	}

	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			mockCtrl := NewMockUserStore(nil)
			server := NewServer(mockCtrl)

			if tt.setupStore {
				server.fileStoreM.Lock()
				server.fileStore[tt.fileID] = map[string]interface{}{
					"nested": map[string]interface{}{
						"key1": "value1",
						"key2": "value2",
					},
				}
				server.fileStoreM.Unlock()
			}

			resp, err := server.DownloadFile(
				t.Context(),
				openapi.DownloadFileRequestObject{
					FileId: tt.fileID,
				},
			)
			require.NoError(t, err)

			// Verify response type
			switch tt.wantResponseType {
			case "DownloadFile200ApplicationoctetStreamResponse":
				respBody, ok := resp.(openapi.DownloadFile200ApplicationoctetStreamResponse)
				assert.True(t, ok, "response should be DownloadFile200ApplicationoctetStreamResponse")

				content, err := io.ReadAll(respBody.Body)
				require.NoError(t, err)
				assert.Equal(t, tt.wantContent, string(content), "content should match expected YAML")

			case "DownloadFile404Response":
				_, ok := resp.(openapi.DownloadFile404Response)
				assert.True(t, ok, "response should be DownloadFile404Response")
			}
		})
	}
}

func createTestFile(t *testing.T, fieldname, filename, content string) *multipart.Reader {
	t.Helper()

	body := new(bytes.Buffer)
	writer := multipart.NewWriter(body)
	part, err := writer.CreateFormFile(fieldname, filename)
	require.NoError(t, err)

	_, err = part.Write([]byte(content))
	require.NoError(t, err)

	err = writer.Close()
	require.NoError(t, err)

	return multipart.NewReader(body, writer.Boundary())
}

func newMultipartReader(filename string, content string) *multipart.Reader {
	body := new(bytes.Buffer)
	writer := multipart.NewWriter(body)
	part, err := writer.CreateFormFile("file", filename)
	if err != nil {
		return nil
	}
	_, err = io.WriteString(part, content)
	if err != nil {
		return nil
	}
	err = writer.Close()
	if err != nil {
		return nil
	}
	return multipart.NewReader(body, writer.Boundary())
}
