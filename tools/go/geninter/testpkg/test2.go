package testpkg

import "fmt"

// Says formats a speech sentence for the user.
func (u *User) Says(str string) string {
	return fmt.Sprintf("%s says: %s", u.name, str)
}
