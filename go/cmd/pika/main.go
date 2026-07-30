// Package main implements a simple CLI tool to generate a local test PKI.
package main

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"fmt"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"strings"
	"time"

	"charm.land/huh/v2"
)

// PKIConfig holds the options required to generate a local test PKI.
type PKIConfig struct {
	OutputDir string
	CAName    string
	CertCNs   []string
}

// Generate creates the Root CA and all queued certificates on disk.
func (c *PKIConfig) Generate() error {
	outputDir := c.OutputDir
	if outputDir == "" {
		outputDir = "output"
	}

	caName := c.CAName
	if caName == "" {
		caName = "Root CA"
	}

	if err := os.MkdirAll(outputDir, 0o755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// 1. Root CA
	rootKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return fmt.Errorf("failed to generate root key: %w", err)
	}

	rootSerial, err := genSerial()
	if err != nil {
		return fmt.Errorf("failed to generate root serial: %w", err)
	}

	rootTemplate := x509.Certificate{
		SerialNumber: rootSerial,
		Subject: pkix.Name{
			CommonName: caName,
		},
		NotBefore:             time.Now().Add(-1 * time.Minute),
		NotAfter:              time.Now().AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		IsCA:                  true,
		BasicConstraintsValid: true,
	}

	rootCertBytes, err := x509.CreateCertificate(rand.Reader, &rootTemplate, &rootTemplate, &rootKey.PublicKey, rootKey)
	if err != nil {
		return fmt.Errorf("failed to create root certificate: %w", err)
	}

	if err := saveFile(filepath.Join(outputDir, "root.crt"), "CERTIFICATE", rootCertBytes); err != nil {
		return err
	}

	rootKeyBytes, err := x509.MarshalPKCS8PrivateKey(rootKey)
	if err != nil {
		return fmt.Errorf("failed to marshal root key: %w", err)
	}
	if err := saveFile(filepath.Join(outputDir, "root.key"), "PRIVATE KEY", rootKeyBytes); err != nil {
		return err
	}

	// 2. Leaf Certificates
	for _, cn := range c.CertCNs {
		if strings.ContainsAny(cn, "/\\") {
			return fmt.Errorf("[%s] invalid CN: must not contain path separators", cn)
		}
		key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if err != nil {
			return fmt.Errorf("[%s] failed to generate key: %w", cn, err)
		}

		serial, err := genSerial()
		if err != nil {
			return fmt.Errorf("[%s] failed to generate serial: %w", cn, err)
		}

		template := x509.Certificate{
			SerialNumber: serial,
			Subject: pkix.Name{
				CommonName: cn,
			},
			NotBefore:             time.Now().Add(-1 * time.Minute),
			NotAfter:              time.Now().AddDate(1, 0, 0),
			KeyUsage:              x509.KeyUsageDigitalSignature,
			ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
			BasicConstraintsValid: true,
		}

		if ip := net.ParseIP(cn); ip != nil {
			template.IPAddresses = []net.IP{ip}
		} else {
			template.DNSNames = []string{cn}
		}

		certBytes, err := x509.CreateCertificate(rand.Reader, &template, &rootTemplate, &key.PublicKey, rootKey)
		if err != nil {
			return fmt.Errorf("[%s] failed to create certificate: %w", cn, err)
		}

		if err := saveFile(filepath.Join(outputDir, cn+".crt"), "CERTIFICATE", certBytes); err != nil {
			return err
		}

		keyBytes, err := x509.MarshalPKCS8PrivateKey(key)
		if err != nil {
			return fmt.Errorf("[%s] failed to marshal key: %w", cn, err)
		}

		if err := saveFile(filepath.Join(outputDir, cn+".key"), "PRIVATE KEY", keyBytes); err != nil {
			return err
		}
	}

	fmt.Printf("\nDone. Generated Root CA and %d certificate(s) in './%s/'.\n", len(c.CertCNs), outputDir)
	return nil
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

const (
	actionAdd      = "add"
	actionGenerate = "generate"
	actionExit     = "exit"
)

func run() error {
	config := PKIConfig{
		OutputDir: "output",
		CAName:    "Root CA",
		CertCNs:   []string{},
	}

	fmt.Printf("=====[ PIKA ] =====\n\n")

	for {
		var choice string

		if err := huh.NewForm(
			huh.NewGroup(
				huh.NewSelect[string]().
					Title(fmt.Sprintf("PIKA (%d in queue)", len(config.CertCNs))).
					Options(
						huh.NewOption("Add certificate to queue", actionAdd),
						huh.NewOption("Generate PKI", actionGenerate),
						huh.NewOption("Exit", actionExit),
					).
					Value(&choice),
			),
		).Run(); err != nil {
			return err
		}

		switch choice {

		case actionAdd:
			var cn string
			huh.NewInput().
				Title("Enter CN (e.g. localhost, 127.0.0.1, service.local): ").
				Value(&cn).
				Run()
			cn = strings.TrimSpace(cn)
			if cn != "" {
				config.CertCNs = append(config.CertCNs, cn)
				fmt.Printf("-> Added '%s'\n", cn)
			}

		case actionGenerate:
			if err := config.Generate(); err != nil {
				fmt.Printf("-> Error: %v\n", err)
				continue
			}
			return nil

		case actionExit:
			return nil

		}
	}
}

// saveFile writes the given bytes to a file at the specified path, encoding it as a PEM block with the given type.
func saveFile(path, blockType string, bytes []byte) error {
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		return fmt.Errorf("failed to create file %s: %w", path, err)
	}
	defer file.Close()

	return pem.Encode(file, &pem.Block{
		Type:  blockType,
		Bytes: bytes,
	})
}

// genSerial generates a random positive serial number for certificates.
func genSerial() (*big.Int, error) {
	limit := new(big.Int).Lsh(big.NewInt(1), 128)
	for {
		serial, err := rand.Int(rand.Reader, limit)
		if err != nil {
			return nil, fmt.Errorf("failed to generate random serial: %w", err)
		}
		if serial.Sign() > 0 {
			return serial, nil
		}
	}
}
