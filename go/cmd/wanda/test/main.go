// Package main is a sample app used by wanda-generated code tests.
package main

func main() {
}

type Hero struct {
	ID   int
	Name string
}

var autoIncrID = 0

type Store struct {
	heroes []Hero
}

func (s *Store) GetHeroes() []Hero {
	return s.heroes
}

func (s *Store) AddHero(h Hero) {
	h.ID = autoIncrID
	s.heroes = append(s.heroes, h)
	autoIncrID++
}

func (s *Store) UpdateHero(id int, hero Hero) {
	for i, h := range s.heroes {
		if h.ID == id {
			s.heroes[i] = hero
			return
		}
	}
}

func (s *Store) DeleteHero(id int) {
	for i, h := range s.heroes {
		if h.ID == id {
			s.heroes = append(s.heroes[:i], s.heroes[i+1:]...)
			return
		}
	}
}

type Service struct {
	db Store
}

func NewService(db Store) *Service {
	return &Service{db}
}
