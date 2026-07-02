// Package main decodes hidden bits from base32/base64 padding.
package main

import (
	"flag"
	"fmt"
	"io"
	"math"
	"os"
	"strconv"
	"strings"
)

const (
	base32Char = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
	base64Char = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
)

func unhide(message, sep string, base int) (string, error) {
	var baseChar string

	switch base {
	case 32:
		baseChar = base32Char
	case 64:
		baseChar = base64Char
	default:
		return "", fmt.Errorf("unsupported base %v", base)
	}

	nbits := int(math.Log2(float64(len(baseChar)))) // number of bits used for each value
	wordLen := LCM(nbits, 8)                        // len of a properly padded encoded string in bits
	nbChars := wordLen / nbits                      // number of characters needed for a properly encoded string

	// number of unused bits depending on the number of padding characters
	ubPad := map[int]int{}
	for i := 0; i < nbChars; i++ {
		ubPad[i] = (wordLen - i*nbits) % 8
	}

	res := ""
	for i, line := range strings.Split(message, sep) {
		line = strings.TrimSpace(line)
		if len(line)%nbChars != 0 {
			fmt.Printf("padding error : skipping element %v (%v)\n", i, line)
			continue
		}

		// length of the padding in bytes
		padding := strings.Count(line, "=")
		if padding == 0 {
			continue // no useless bits to read
		}

		// last encoding char of the string, it contains useless bits
		lastChar := line[strings.Index(line, "=")-1]
		// binary value of the last character, left padded with zeroes
		binVal := pad(strconv.FormatInt(int64(strings.Index(baseChar, string(lastChar))), 2), nbits)

		res += binVal[len(binVal)-ubPad[padding]:]
	}

	return binaryToString(res), nil
}

func binaryToString(bin string) string {
	str := ""
	for i := 0; i < len(bin); i += 8 {
		if i+8 > len(bin) {
			break
		}
		s, err := strconv.ParseInt(bin[i:i+8], 2, 32)
		if err != nil {
			panic(err)
		}
		str += string(byte(s))
	}
	return str
}

func pad(s string, length int) string {
	for len(s) < length {
		s = "0" + s
	}
	return s
}

func GCD(a, b int) int {
	for b != 0 {
		t := b
		b = a % b
		a = t
	}
	return a
}

func LCM(a, b int, integers ...int) int {
	result := a * b / GCD(a, b)

	for i := 0; i < len(integers); i++ {
		result = LCM(result, integers[i])
	}

	return result
}

func main() {
	fileIn := flag.String("in", "", "file with encoded strings")
	sep := flag.String("sep", "\n", "separator character of encoded strings in")
	base := flag.Int("base", 64, "encoding base (32 or 64)")

	flag.Parse()

	fmt.Printf("Read file %v\n", *fileIn)

	file, err := os.Open(*fileIn)
	if err != nil {
		fmt.Println("error:", err)
		return
	}
	defer file.Close()

	datas, err := io.ReadAll(file)
	if err != nil {
		fmt.Println("error:", err)
		return
	}

	fmt.Println("Searching for hidden datas")
	res, err := unhide(string(datas), *sep, *base)
	if err != nil {
		fmt.Println("error:", err)
		return
	}

	fmt.Printf("\n%v\n", res)
}
