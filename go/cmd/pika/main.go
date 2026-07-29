package main

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"net"
	"os"
	"time"
)

func main() {
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

	rootCertFile, err := os.Create("root.crt")
	if err != nil {
		panic(err)
	}

	pem.Encode(rootCertFile, &pem.Block{
		Type:  "CERTIFICATE",
		Bytes: rootCertBytes,
	})

	rootCertFile.Close()

	for _, cn := range os.Args[1:] {
		key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if err != nil {
			panic(err)
		}

		template := x509.Certificate{
			SerialNumber: genSerial(),
			Subject: pkix.Name{
				CommonName: cn,
			},
			NotBefore:   time.Now(),
			NotAfter:    time.Now().AddDate(1, 0, 0),
			IPAddresses: []net.IP{net.ParseIP("127.0.0.1")},
		}

		certBytes, err := x509.CreateCertificate(rand.Reader, &template, &rootTemplate, &key.PublicKey, rootKey)
		if err != nil {
			panic(err)
		}

		certFile, err := os.Create(cn + ".crt")
		if err != nil {
			panic(err)
		}

		pem.Encode(certFile, &pem.Block{
			Type:  "CERTIFICATE",
			Bytes: certBytes,
		})

		certFile.Close()

		keyFile, err := os.Create(cn + ".key")
		if err != nil {
			panic(err)
		}

		keyBytes, err := x509.MarshalECPrivateKey(key)
		if err != nil {
			panic(err)
		}

		pem.Encode(keyFile, &pem.Block{
			Type:  "EC PRIVATE KEY",
			Bytes: keyBytes,
		})

		keyFile.Close()
	}
}

func genSerial() *big.Int {
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		panic(err)
	}
	return serial
}
