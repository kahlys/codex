// Package message defines tea message types used by the cemetery app.
package message

// Error wraps an error as a tea message.
type Error struct {
	err error
}

// NewError creates an Error message.
func NewError(err error) Error {
	return Error{err}
}

func (e Error) Error() string {
	return e.err.Error()
}
