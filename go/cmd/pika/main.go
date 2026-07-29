package main

import (
	"bufio"
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
)

func main() {
	var stack []string
	scanner := bufio.NewScanner(os.Stdin)

	for {
		fmt.Println("\n--- PIKA MENU ---")
		fmt.Println("1. Add certificate to queue")
		fmt.Println("2. Generate PKI")
		fmt.Println("3. Exit")
		fmt.Printf("Queued certificates (%d): %v\n", len(stack), stack)
		fmt.Print("Choice: ")

		if !scanner.Scan() {
			break
		}

		switch strings.TrimSpace(scanner.Text()) {
		case "1":
			fmt.Print("Enter CN (e.g. localhost, 127.0.0.1, service.local): ")
			if scanner.Scan() {
				cn := strings.TrimSpace(scanner.Text())
				if cn != "" {
					stack = append(stack, cn)
					fmt.Printf("-> Added '%s'\n", cn)
				}
			}
		case "2":
			if len(stack) == 0 {
				fmt.Println("-> Queue is empty. Add at least one CN.")
				continue
			}
			generatePKI(stack)
			return
		case "3":
			fmt.Println("Exiting.")
			return
		default:
			fmt.Println("Invalid choice.")
		}
	}
}

func generatePKI(stack []string) {
	outputDir := "output"
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		panic(err)
	}

	rootKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		panic(err)
	}

	rootTemplate := x509.Certificate{
		SerialNumber: genSerial(),
		Subject: pkix.Name{
			CommonName: "Root CA",
		},
		NotBefore:             time.Now(),
		NotAfter:              time.Now().AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		IsCA:                  true,
		BasicConstraintsValid: true,
	}

	rootCertBytes, err := x509.CreateCertificate(rand.Reader, &rootTemplate, &rootTemplate, &rootKey.PublicKey, rootKey)
	if err != nil {
		panic(err)
	}

	saveFile(filepath.Join(outputDir, "root.crt"), "CERTIFICATE", rootCertBytes)

	rootKeyBytes, err := x509.MarshalECPrivateKey(rootKey)
	if err != nil {
		panic(err)
	}
	saveFile(filepath.Join(outputDir, "root.key"), "EC PRIVATE KEY", rootKeyBytes)

	for _, cn := range stack {
		key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if err != nil {
			panic(err)
		}

		template := x509.Certificate{
			SerialNumber: genSerial(),
			Subject: pkix.Name{
				CommonName: cn,
			},
			NotBefore:             time.Now(),
			NotAfter:              time.Now().AddDate(1, 0, 0),
			KeyUsage:              x509.KeyUsageDigitalSignature,
			ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
			BasicConstraintsValid: true,
		}

		if ip := net.ParseIP(cn); ip != nil {
			template.IPAddresses = []net.IP{ip}
		} else {
			template.DNSNames = []string{cn}
			template.IPAddresses = []net.IP{net.ParseIP("127.0.0.1")}
		}

		certBytes, err := x509.CreateCertificate(rand.Reader, &template, &rootTemplate, &key.PublicKey, rootKey)
		if err != nil {
			panic(err)
		}

		saveFile(filepath.Join(outputDir, cn+".crt"), "CERTIFICATE", certBytes)

		keyBytes, err := x509.MarshalECPrivateKey(key)
		if err != nil {
			panic(err)
		}

		saveFile(filepath.Join(outputDir, cn+".key"), "EC PRIVATE KEY", keyBytes)
	}

	fmt.Printf("\nDone. Generated Root CA and %d certificate(s) in './%s/'.\n", len(stack), outputDir)
}

func saveFile(path, blockType string, bytes []byte) {
	file, err := os.Create(path)
	if err != nil {
		panic(err)
	}
	defer file.Close()

	pem.Encode(file, &pem.Block{
		Type:  blockType,
		Bytes: bytes,
	})
}

func genSerial() *big.Int {
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		panic(err)
	}
	return serial
}
