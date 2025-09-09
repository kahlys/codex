package main

import (
	"bytes"
	"flag"
	"fmt"
	"go/format"
	"html/template"
	"log"
	"os"
	"strings"

	"github.com/getkin/kin-openapi/openapi3"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

func main() {
	log.SetFlags(log.Flags() &^ (log.Ldate | log.Ltime))

	specFile := flag.String("file", "", "an openapi file")
	flag.Parse()

	if *specFile == "" {
		log.Fatal("ERROR: missing argument '-file'")
	}

	doc, err := openapi3.NewLoader().LoadFromFile(*specFile)
	if err != nil {
		log.Fatal("ERROR: ", err)
	}

	myDoc := Doc{
		Title: doc.Info.Title,
	}
	methods := []string{"GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE"}
	for path, item := range doc.Paths.Map() {
		for _, method := range methods {
			if op := item.GetOperation(method); op != nil {
				if op.OperationID == "" {
					log.Fatal("ERROR: OperationID is empty")
				}

				name := cases.Title(language.English).String(op.OperationID)

				// generate param out
				gparamIn := GParam{Name: name}
				for _, param := range op.Parameters {
					p := Param{
						Name: param.Value.Name,
					}
					switch schemaType := param.Value.Schema.Value.Type; {
					case schemaType.Is("string"):
						p.Type = "string"
					case schemaType.Is("integer"):
						p.Type = "int"
					case schemaType.Is("number"):
						p.Type = "float64"
					case schemaType.Is("boolean"):
						p.Type = "bool"
					default:
						log.Fatal("ERROR: unknown type: ", param.Value.Schema.Value.Type)
					}
					switch param.Value.In {
					case "query":
						gparamIn.Parameters.InPath = append(gparamIn.Parameters.InPath, p)
					case "path":
						gparamIn.Parameters.InPath = append(gparamIn.Parameters.InPath, p)
					}
				}

				// generate param body
				if op.RequestBody != nil {
					for c, v := range op.RequestBody.Value.Content {
						switch c {
						case "application/json":
							gparamIn.Body = SchemaAsJSON(0, v.Schema.Value)
						default:
							log.Print("Warning: unsupported content type: ", c)
						}
					}
				}

				fmt.Println(gparamIn.Body)

				paramIn := &bytes.Buffer{}
				if err := templParam.Execute(paramIn, gparamIn); err != nil {
					log.Fatal(err)
				}

				myDoc.Routes = append(myDoc.Routes, Route{
					Path:        path,
					Method:      method,
					Handler:     "nil",
					Name:        name,
					ParamInCode: paramIn.String(),
				})
			}
		}
	}

	// fmt.Println(myDoc)

	// generate code
	buf := &bytes.Buffer{}
	if err := templ.Execute(buf, myDoc); err != nil {
		log.Fatal(err)
	}

	// format generated code
	pretty, err := format.Source(buf.Bytes())
	if err != nil {
		log.Fatal("fmt", err)
	}

	// write genrerated code
	os.Mkdir("out", os.ModePerm)
	genFile, err := os.Create("out/openapi.go")
	if err != nil {
		log.Fatal(err)
	}
	_, err = genFile.Write(pretty)
	if err != nil {
		log.Fatal(err)
	}
}

type Doc struct {
	Title  string
	Routes []Route
}

type Route struct {
	Name    string
	Method  string
	Path    string
	Handler string

	ParamInCode string
}

type GParam struct {
	Name       string
	Parameters Parameters
	Body       string
}

type Parameters struct {
	InPath  []Param
	InQuery []Param
}

type Param struct {
	Name string
	Type string
}

var templParam = template.Must(template.New("param").Parse(`
type {{ .Name }}Request struct {
	{{ if .Parameters.InPath -}}
	ParamPath struct {
		{{- range .Parameters.InPath }}
		{{ .Name }} {{ .Type }}
		{{- end }}
	}
	{{- end }}
	{{ if .Parameters.InQuery -}}
	ParamQuery struct {
		{{- range .Parameters.InQuery }}
		{{ .Name }} {{ .Type }}
		{{- end }}
	}
	{{- end }}
	{{ if .Body -}}
	Body {{ .Body }}
	{{- end }}
}
`))

var templ = template.Must(template.New("gen").Parse(tmplPkg))

var tmplPkg = `
{{- $title := .Title -}}
package openapi

import (
	"context"
	"encoding/json"
	"net/http"

	"github.com/gorilla/mux"
)

type {{ $title }}Service interface {
	{{ range .Routes -}}
	{{ .Name }}(context.Context, *{{ .Name }}Request) (*{{ .Name }}Response, error)
	{{ end -}}
}

type {{ $title }}Server struct {
	s {{ $title }}Service
}

func New{{ $title }}Server(s {{ $title }}Service) *{{ $title }}Server {
	return &{{ $title }}Server{s: s}
}

func (s *{{ $title }}Server) Handler() http.Handler {
	r := mux.NewRouter()
	{{ range .Routes -}}
	r.HandleFunc("{{ .Path }}", s.{{ .Name }}).Methods("{{ .Method }}")
	{{ end -}}
	return r
}

{{ range .Routes }}
{{ .ParamInCode }}

type {{ .Name }}Response struct {
}

func (s *{{ $title }}Server) {{ .Name }}(w http.ResponseWriter, r *http.Request) {
	resp, err := s.s.{{ .Name }}(r.Context(), nil)
	if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }
	if err := json.NewEncoder(w).Encode(resp); err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }
}
{{ end }}
`

const (
	jsonNil = "nil"
)

func SchemaAsJSON(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	switch schemaType := schema.Type; {
	case schemaType.Is("object"):
		return SchemaAsJSONObject(ident, schema)
	case schemaType.Is("array"):
		return SchemaAsJSONArray(ident, schema)
	case schemaType.Is("string"):
		return SchemaAsJSONString(ident, schema)
	case schemaType.Is("integer"), schemaType.Is("number"):
		return SchemaAsJSONInt(ident, schema)
	case schemaType.Is("boolean"):
		return SchemaAsJSONBool(ident, schema)
	default:
		return ""
	}
}

func SchemaAsJSONObject(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	res := fmt.Sprintf("%vstruct {\n", strings.Repeat("  ", ident))
	first := true
	ident++
	for name, schema := range schema.Properties {
		if !first {
			res += "\n"
		}
		value := strings.TrimSpace(SchemaAsJSON(ident, schema.Value))
		res += fmt.Sprintf("%v%v %v `json:\"%vomitempty\"`", strings.Repeat("  ", ident), cases.Title(language.English).String(name), value, name)
		first = false
	}
	res += fmt.Sprintf("\n%v}", strings.Repeat("  ", ident-1))
	return res
}

func SchemaAsJSONArray(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	// schema.Items.Value.Type to get which type is in array
	return "[\n" +
		SchemaAsJSON(ident+1, schema.Items.Value) +
		fmt.Sprintf("\n%v]", strings.Repeat("  ", ident))
}

func SchemaAsJSONString(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	return fmt.Sprintf(`%vstring`, strings.Repeat("  ", ident))
}

func SchemaAsJSONInt(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	return fmt.Sprintf(`%vint`, strings.Repeat("  ", ident))
}

func SchemaAsJSONBool(ident int, schema *openapi3.Schema) string {
	if schema == nil {
		return jsonNil
	}
	return fmt.Sprintf(`%vbool`, strings.Repeat("  ", ident))
}
