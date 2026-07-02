// Package main generates an interface from methods of a target struct.
package main

import (
	"bytes"
	"flag"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/printer"
	"go/token"
	"os"
	"sort"
	"strings"
)

func main() {
	packagePath := flag.String("package", "", "package path")
	structName := flag.String("struct", "", "struct name")
	flag.Parse()

	check(*packagePath == "", "Package path is required")
	check(*structName == "", "Struct name is required")

	res, err := run(*packagePath, *structName)
	checkErr(err, "Error running")

	fmt.Println(res)
}

func run(pkgPath string, structName string) (string, error) {
	//nolint:staticcheck // parser.ParseDir is sufficient for this lightweight generator.
	pkgs, err := parser.ParseDir(
		token.NewFileSet(),
		pkgPath,
		func(info os.FileInfo) bool {
			return !strings.HasSuffix(info.Name(), "_test.go")
		},
		parser.ParseComments,
	)
	if err != nil {
		return "", err
	}
	if len(pkgs) == 0 {
		return "", fmt.Errorf("no packages found")
	}
	if len(pkgs) > 1 {
		return "", fmt.Errorf("multiple packages found")
	}

	var methods []string
	for _, pkg := range pkgs {
		for _, file := range pkg.Files {
			methods = append(methods, getMethods(structName, file)...)
		}
	}

	return generate(structName, methods)
}

func getMethods(structName string, file *ast.File) []string {
	var methods []string
	ast.Inspect(file, func(node ast.Node) bool {
		funcDecl, ok := node.(*ast.FuncDecl)
		if !ok {
			return true
		}
		if funcDecl.Recv == nil {
			return true
		}
		for _, field := range funcDecl.Recv.List {
			starExpr, ok := field.Type.(*ast.StarExpr)
			if !ok {
				continue
			}
			ident, ok := starExpr.X.(*ast.Ident)
			if !ok {
				continue
			}
			if ident.Name == structName {
				method := fmt.Sprintf(
					"%s(%s) (%s)",
					funcDecl.Name.Name,
					fieldsToString(funcDecl.Type.Params),
					fieldsToString(funcDecl.Type.Results),
				)
				methods = append(methods, method)
			}
		}
		return true
	})
	return methods
}

func generate(structName string, methods []string) (string, error) {
	buf := bytes.Buffer{}

	sort.Strings(methods)

	fmt.Fprintf(&buf, "type %s interface {\n", structName)
	for _, method := range methods {
		fmt.Fprintf(&buf, "\t%s\n", method)
	}
	fmt.Fprintf(&buf, "}")

	pretty, err := format.Source(buf.Bytes())
	if err != nil {
		return "", err
	}

	return string(pretty), nil
}

func fieldsToString(fieldList *ast.FieldList) string {
	if fieldList == nil {
		return ""
	}

	strs := []string{}
	for _, field := range fieldList.List {
		if field.Type == nil {
			continue
		}
		fieldType := typeExprToString(field.Type)
		if len(field.Names) > 0 {
			for _, name := range field.Names {
				strs = append(strs, fmt.Sprintf("%s %s", name, fieldType))
			}
		} else {
			strs = append(strs, fieldType)
		}
	}
	return strings.Join(strs, ", ")
}

func typeExprToString(expr ast.Expr) string {
	var buf bytes.Buffer
	printer.Fprint(&buf, token.NewFileSet(), expr)
	return buf.String()
}

func checkErr(err error, msg string) {
	if err != nil {
		fmt.Printf("%s\n%s\n", msg, err)
		os.Exit(1)
	}
}

func check(b bool, msg string) {
	if b {
		fmt.Println(msg)
		os.Exit(1)
	}
}
