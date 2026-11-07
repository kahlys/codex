package alfred

import (
	"net/url"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestParseFilterFromURLValues(t *testing.T) {
	tests := map[string]struct {
		url  string
		want Filter
	}{
		"like":    {url: "?filter[name][like]=batman", want: Like{"name", "batman"}},
		"eq":      {url: "?filter[name][eq]=batman", want: EQ{"name", "batman"}},
		"gt":      {url: "?filter[age][gt]=42", want: GT{"age", "42"}},
		"gte":     {url: "?filter[age][gte]=42", want: GTE{"age", "42"}},
		"lt":      {url: "?filter[age][lt]=42", want: LT{"age", "42"}},
		"lte":     {url: "?filter[age][lte]=42", want: LTE{"age", "42"}},
		"contain": {url: "?filter[age][contain]=42", want: Contain{"age", "42"}},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			url, err := url.Parse(tt.url)
			assert.NoError(t, err)
			f := ParseURLValues(url.Query())
			assert.Equal(t, tt.want, f.Filters[0])
		})
	}
}

func TestToSQL(t *testing.T) {
	tests := map[string]struct {
		filter Filter
		want   string
	}{
		"like": {filter: Like{"name", "batman"}, want: `"name" ILIKE '%batman%'`},
		"eq":   {filter: EQ{"name", "batman"}, want: `"name" = 'batman'`},
		"gt":   {filter: GT{"age", "42"}, want: `"age" > '42'`},
		"gte":  {filter: GTE{"age", "42"}, want: `"age" >= '42'`},
		"lt":   {filter: LT{"age", "42"}, want: `"age" < '42'`},
		"lte":  {filter: LTE{"age", "42"}, want: `"age" <= '42'`},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			actual := tt.filter.ToSQL()
			assert.Equal(t, tt.want, actual)
		})
	}
}

func TestKeep(t *testing.T) {
	data := struct {
		Name  string `filter:"name"`
		Alive bool   `filter:"alive"`
		Age   int    `filter:"age"`
	}{
		Name:  "Bruce,Wayne",
		Alive: true,
		Age:   39,
	}
	tests := map[string]struct {
		filter Filter
		want   bool
	}{
		"like-keep":     {filter: Like{"name", "bruce"}, want: true},
		"like-drop":     {filter: Like{"name", "diana"}, want: false},
		"eq-keep":       {filter: EQ{"alive", "1"}, want: true},
		"eq-drop":       {filter: EQ{"alive", "0"}, want: false},
		"gt-keep":       {filter: GT{"age", "32"}, want: true},
		"gt-drop":       {filter: GT{"age", "39"}, want: false},
		"gte-keep":      {filter: GTE{"age", "39"}, want: true},
		"gte-drop":      {filter: GTE{"age", "42"}, want: false},
		"lt-keep":       {filter: LT{"age", "42"}, want: true},
		"lt-drop":       {filter: LT{"age", "39"}, want: false},
		"lte-keep":      {filter: LTE{"age", "39"}, want: true},
		"lte-drop":      {filter: LTE{"age", "24"}, want: false},
		"contains-keep": {filter: Contain{"name", "Bruce"}, want: true},
		"contains-drop": {filter: Contain{"name", "Diana"}, want: false},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			actual, err := tt.filter.Keep(data)
			assert.NoError(t, err)
			assert.Equal(t, tt.want, actual)
		})
	}
}

func TestParseSortAndPageFromURLValues(t *testing.T) {
	endpoint := "?offset=1&limit=2&orderBy=ASC&sortBy=name"
	want := Option{Offset: 1, Limit: 2, SortBy: "name", Order: "asc"}
	url, err := url.Parse(endpoint)
	assert.NoError(t, err)
	f := ParseURLValues(url.Query())
	assert.Equal(t, want, f)
}

func TestParseURLValuesOrderIsWhitelisted(t *testing.T) {
	tests := map[string]struct {
		values url.Values
		want   string
	}{
		"asc":         {values: url.Values{"orderBy": {"ASC"}}, want: "asc"},
		"desc":        {values: url.Values{"orderBy": {"desc"}}, want: "desc"},
		"desc-upper":  {values: url.Values{"orderBy": {"DESC"}}, want: "desc"},
		"absent":      {values: url.Values{}, want: ""},
		"injection":   {values: url.Values{"orderBy": {"asc; DROP TABLE users; --"}}, want: ""},
		"unknown":     {values: url.Values{"orderBy": {"sideways"}}, want: ""},
		"union-query": {values: url.Values{"orderBy": {"asc) UNION SELECT 1 --"}}, want: ""},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			assert.Equal(t, tt.want, ParseURLValues(tt.values).Order)
		})
	}
}

func TestAddToPSQLQuery(t *testing.T) {
	const baseQuery = "SELECT * FROM users"
	tests := map[string]struct {
		opt  Option
		want string
	}{
		"no-option": {
			opt:  Option{},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query  LIMIT ALL OFFSET 0`,
		},
		"filters-sort-pagination": {
			opt: Option{
				Limit:   10,
				Offset:  5,
				SortBy:  "name",
				Order:   "DESC",
				Filters: Filters{Like{"name", "bat"}, EQ{"age", "42"}},
			},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query WHERE "name" ILIKE '%bat%' AND "age" = '42' ORDER BY "name" desc LIMIT 10 OFFSET 5`,
		},
		"negative-limit-offset": {
			opt:  Option{Limit: -1, Offset: -2},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query  LIMIT ALL OFFSET 0`,
		},
		"sort-without-order": {
			opt:  Option{Limit: 2, SortBy: "name"},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query  ORDER BY "name" LIMIT 2 OFFSET 0`,
		},
		"order-injection": {
			opt:  Option{SortBy: "name", Order: "asc; DROP TABLE users; --"},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query  ORDER BY "name" LIMIT ALL OFFSET 0`,
		},
		"sortby-injection-stays-quoted-identifier": {
			opt:  Option{SortBy: `name"; DROP TABLE users; --`},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query  ORDER BY "name""; DROP TABLE users; --" LIMIT ALL OFFSET 0`,
		},
		"value-injection-is-quoted": {
			opt:  Option{Filters: Filters{EQ{"name", "batman'; DROP TABLE users; --"}}},
			want: `SELECT *, count(*) OVER() FROM (SELECT * FROM users) AS query WHERE "name" = 'batman''; DROP TABLE users; --' LIMIT ALL OFFSET 0`,
		},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			assert.Equal(t, tt.want, AddToPSQLQuery(baseQuery, tt.opt))
		})
	}
}

func TestAddToPSQLQueryNeverEmitsInvalidOrder(t *testing.T) {
	tests := map[string]struct {
		order string
		want  string
	}{
		"empty":       {order: "", want: `ORDER BY "name" LIMIT`},
		"asc":         {order: "asc", want: `ORDER BY "name" asc LIMIT`},
		"desc":        {order: "DESC", want: `ORDER BY "name" desc LIMIT`},
		"injection":   {order: "asc; DROP TABLE users; --", want: `ORDER BY "name" LIMIT`},
		"union-query": {order: "asc) UNION SELECT 1 --", want: `ORDER BY "name" LIMIT`},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			got := AddToPSQLQuery("SELECT * FROM users", Option{SortBy: "name", Order: tt.order})
			assert.Contains(t, got, tt.want)
			assert.NotContains(t, strings.ToLower(got), "drop table")
		})
	}
}
