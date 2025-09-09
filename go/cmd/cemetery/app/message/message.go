package message

type Error struct {
	err error
}

func NewError(err error) Error {
	return Error{err}
}

func (e Error) Error() string {
	return e.err.Error()
}
