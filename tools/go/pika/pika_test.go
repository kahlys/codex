package main

import (
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPKIFilesExist(t *testing.T) {
	t.Parallel()

	dir, _ := setupTestPKI(t, "localhost", "127.0.0.1")

	expectedFiles := []string{
		"root.crt", "root.key",
		"localhost.crt", "localhost.key",
		"127.0.0.1.crt", "127.0.0.1.key",
	}

	for _, file := range expectedFiles {
		require.FileExists(t, filepath.Join(dir, file))
	}
}

func TestTLSConnection(t *testing.T) {
	t.Parallel()

	cns := []string{"localhost", "127.0.0.1"}
	dir, pool := setupTestPKI(t, cns...)

	for _, cn := range cns {
		cn := cn
		t.Run(cn, func(t *testing.T) {
			t.Parallel()

			ts, client := newTLSServerAndClient(t, dir, pool, cn)
			defer ts.Close()

			resp, err := client.Get(ts.URL)
			require.NoError(t, err)
			defer resp.Body.Close()

			body, err := io.ReadAll(resp.Body)
			require.NoError(t, err)

			assert.Equal(t, http.StatusOK, resp.StatusCode)
			assert.Equal(t, "TLS Handshake Successful\n", string(body))
		})
	}
}

func setupTestPKI(t *testing.T, cns ...string) (string, *x509.CertPool) {
	t.Helper()

	dir := t.TempDir()

	config := PKIConfig{
		OutputDir: dir,
		CAName:    "Test Root CA",
		CertCNs:   cns,
	}

	err := config.Generate()
	require.NoError(t, err)

	rootCertPEM, err := os.ReadFile(filepath.Join(dir, "root.crt"))
	require.NoError(t, err)

	pool := x509.NewCertPool()
	require.True(t, pool.AppendCertsFromPEM(rootCertPEM))

	return dir, pool
}

func newTLSServerAndClient(t *testing.T, dir string, pool *x509.CertPool, cn string) (*httptest.Server, *http.Client) {
	t.Helper()

	certPath := filepath.Join(dir, cn+".crt")
	keyPath := filepath.Join(dir, cn+".key")

	cert, err := tls.LoadX509KeyPair(certPath, keyPath)
	require.NoError(t, err)

	handler := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		fmt.Fprintln(w, "TLS Handshake Successful")
	})

	ts := httptest.NewUnstartedServer(handler)
	ts.TLS = &tls.Config{Certificates: []tls.Certificate{cert}}
	ts.StartTLS()

	client := ts.Client()
	if transport, ok := client.Transport.(*http.Transport); ok {
		transport.TLSClientConfig.RootCAs = pool
		transport.TLSClientConfig.ServerName = cn
	}

	return ts, client
}
