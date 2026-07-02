// Package cryptox provides decryption helpers used by oracle.
package cryptox

import (
	"crypto/aes"
	"crypto/cipher"
	"fmt"
)

// AesGCMDecrypt decrypts AES-GCM ciphertext prefixed with its nonce.
func AesGCMDecrypt(in []byte, key []byte) ([]byte, error) {
	if len(key) == 0 {
		return []byte{}, fmt.Errorf("aes-gcm decryption failed: missing or empty encryption key")
	}

	c, err := aes.NewCipher(key)
	if err != nil {
		return []byte{}, fmt.Errorf("aes-gcm decryption failed: %w", err)
	}
	gcm, err := cipher.NewGCM(c)
	if err != nil {
		return []byte{}, fmt.Errorf("aes-gcm decryption failed: %w", err)
	}
	nonceSize := gcm.NonceSize()

	if len(in) < nonceSize {
		return []byte{}, fmt.Errorf("aes-gcm decryption failed: ciphertext length exceeds nonce size")
	}

	nonce, ciphertext := in[:nonceSize], in[nonceSize:]
	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return []byte{}, fmt.Errorf("aes-gcm decryption failed: %w", err)
	}

	return plaintext, nil
}
