//go:build windows

package cryptox

import (
	"syscall"
	"unsafe"
)

// WDataBlob represents encrypted data with the Windows APIs
// CryptProtectData and CryptUnprotectData.
type WDataBlob struct {
	CbData uint32
	PbData *byte
}

func NewBlob(d []byte) *WDataBlob {
	if len(d) == 0 {
		return &WDataBlob{}
	}
	return &WDataBlob{
		PbData: &d[0],
		CbData: uint32(len(d)),
	}
}

func (b *WDataBlob) Bytes() []byte {
	d := make([]byte, b.CbData)
	copy(d, (*[1 << 30]byte)(unsafe.Pointer(b.PbData))[:])
	return d
}

var (
	procDecryptData = syscall.NewLazyDLL("Crypt32.dll").NewProc("CryptUnprotectData")
	procLocalFree   = syscall.NewLazyDLL("Kernel32.dll").NewProc("LocalFree")
)

// WDecrypt decrypts the data encrypted with Windows APIs CryptProtectData.
func WDecrypt(data []byte) ([]byte, error) {
	var outblob WDataBlob
	r, _, err := procDecryptData.Call(uintptr(unsafe.Pointer(NewBlob(data))), 0, 0, 0, 0, 0, uintptr(unsafe.Pointer(&outblob)))
	if r == 0 {
		return nil, err
	}
	defer procLocalFree.Call(uintptr(unsafe.Pointer(outblob.PbData)))
	return outblob.Bytes(), nil
}
