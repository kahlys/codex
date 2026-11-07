// Package testpkg contains fixtures used by geninter tests.
package testpkg

import "context"

// User is a sample entity used by generator tests.
type User struct {
	name    string
	surname string
	age     int
}

// Name returns the first name.
func (u *User) Name() string {
	return u.name
}

// SetName sets the first name.
func (u *User) SetName(name string) {
	u.name = name
}

// SetNames sets first and last names.
func (u *User) SetNames(name, surname string) {
	u.name = name
	u.surname = surname
}

// Names returns first and last names.
func (u *User) Names() (name, surname string) {
	return u.name, u.surname
}

// Age returns the age.
func (u *User) Age() int {
	return u.age
}

// SetAge sets the age.
func (u *User) SetAge(age int) {
	u.age = age
}

// Info returns a subset of user information.
func (u *User) Info() (name string, age int) {
	return u.name, u.age
}

// SetInfo sets a subset of user information.
func (u *User) SetInfo(name string, age int) {
	u.name = name
	u.age = age
}

// IsAdult reports whether the user is an adult.
func (u *User) IsAdult(_ context.Context) bool {
	return u.age >= 18
}
