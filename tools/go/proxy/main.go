// Package main runs a configurable TCP/TLS proxy.
package main

import (
	"crypto/tls"
	"flag"
	"log"

	"github.com/kahlys/codex/go/cmd/proxy/internal/proxy"
)

var (
	// addresses
	localAddr  = flag.String("lhost", ":4444", "proxy local address")
	targetAddr = flag.String("rhost", ":80", "proxy remote address")

	// tls configuration for proxy as a server (listen)
	localTLS  = flag.Bool("ltls", false, "tls/ssl between client and proxy, you must set 'lcert' and 'lkey'")
	localCert = flag.String("lcert", "", "certificate file for proxy server side")
	localKey  = flag.String("lkey", "", "key x509 file for proxy server side")

	// tls configuration for proxy as a client (connection to target)
	targetTLS  = flag.Bool("rtls", false, "tls/ssl between proxy and target, you must set 'rcert' and 'rkey'")
	targetCert = flag.String("rcert", "", "certificate file for proxy client side")
	targetKey  = flag.String("rkey", "", "key x509 file for proxy client side")
)

func main() {
	flag.Parse()

	p := proxy.Server{
		Addr:   *localAddr,
		Target: *targetAddr,
	}

	if *targetTLS {
		cert, err := tls.LoadX509KeyPair(*targetCert, *targetKey)
		if err != nil {
			log.Fatalf("configuration tls for target connection: %v", err)
		}
		p.TLSConfigTarget = &tls.Config{Certificates: []tls.Certificate{cert}, InsecureSkipVerify: true}
	}

	log.Println("Proxying from " + p.Addr + " to " + p.Target)
	if *localTLS {
		p.ListenAndServeTLS(*localCert, *localKey)
	} else {
		p.ListenAndServe()
	}
}
