package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"regexp"
	"strings"
	"time"

	"github.com/jedib0t/go-pretty/table"
	"github.com/jedib0t/go-pretty/text"
	"github.com/spf13/cobra"
)

type OSVScannerResult struct {
	Results []struct {
		Source struct {
			Path string `json:"path"`
			Type string `json:"type"`
		} `json:"source"`
		Packages []struct {
			Package struct {
				Name      string `json:"name"`
				Version   string `json:"version"`
				Ecosystem string `json:"ecosystem"`
			} `json:"package"`
			Vulnerabilities []struct {
				SchemaVersion string    `json:"schema_version"`
				ID            string    `json:"id"`
				Modified      time.Time `json:"modified"`
				Published     time.Time `json:"published"`
				Aliases       []string  `json:"aliases"`
				Summary       string    `json:"summary"`
				Details       string    `json:"details"`
				Affected      []struct {
					Package struct {
						Ecosystem string `json:"ecosystem"`
						Name      string `json:"name"`
						Purl      string `json:"purl"`
					} `json:"package"`
					Ranges []struct {
						Type   string `json:"type"`
						Events []struct {
							Introduced string `json:"introduced,omitempty"`
							Fixed      string `json:"fixed,omitempty"`
						} `json:"events"`
					} `json:"ranges"`
					DatabaseSpecific struct {
						Source string `json:"source"`
					} `json:"database_specific"`
				} `json:"affected"`
				References []struct {
					Type string `json:"type"`
					URL  string `json:"url"`
				} `json:"references"`
				DatabaseSpecific struct {
					CweIDs         []string `json:"cwe_ids"`
					GithubReviewed bool     `json:"github_reviewed"`
					Severity       string   `json:"severity"`
				} `json:"database_specific"`
			} `json:"vulnerabilities"`
			Groups []struct {
				IDs []string `json:"ids"`
			} `json:"groups"`
		} `json:"packages"`
	} `json:"results"`
}

var fIn string

var results OSVScannerResult

func main() {
	root := &cobra.Command{
		Use:     "osv-viewer",
		Short:   "OSV Scanner viewer",
		Version: "1",
		CompletionOptions: cobra.CompletionOptions{
			DisableDefaultCmd: true,
		},
		PersistentPreRunE: func(*cobra.Command, []string) error {
			content, err := os.ReadFile(fIn)
			if err != nil {
				return err
			}

			return json.Unmarshal(content, &results)
		},
	}

	root.AddCommand(
		&cobra.Command{
			Use:   "sources",
			Short: "List sources",
			Run:   runSourcesCommand,
		},
		&cobra.Command{
			Use:   "show [source id]",
			Short: "List all vulnerabilities of a source",
			Args:  cobra.MatchAll(cobra.ExactArgs(1)),
			RunE:  runShowCommand,
		},
	)

	root.PersistentFlags().StringVar(&fIn, "in", "", "osv scanner output json file")
	root.MarkPersistentFlagRequired("in")

	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}

func runSourcesCommand(_ *cobra.Command, _ []string) {
	total := 0

	t := newTable()
	t.AppendHeader(table.Row{"source id", "source", "total"})
	t.SortBy([]table.SortBy{{Name: "total", Mode: table.DscNumeric}})

	for _, r := range results.Results {
		vuln := 0
		for _, rp := range r.Packages {
			vuln += len(rp.Vulnerabilities)
			total += len(rp.Vulnerabilities)
		}
		t.AppendRow(table.Row{hash(r.Source.Path), r.Source.Path, fmt.Sprintf("%v", vuln)})
	}
	fmt.Printf("Total of vulnerabilities: %v\n\n", total)
	fmt.Println(t.Render())
}

var rg = regexp.MustCompile(`(\r\n?|\n){2,}`)

func runShowCommand(_ *cobra.Command, args []string) error {
	for _, r := range results.Results {
		// find sources
		if hash(r.Source.Path) != args[0] {
			continue
		}
		fmt.Printf("Source: %v\n", r.Source.Path)

		for _, rp := range r.Packages {
			pkg := fmt.Sprintf("%v %v", rp.Package.Name, rp.Package.Version)

			for _, vuln := range rp.Vulnerabilities {
				// find fixed version
				fixed := []string{}
				for _, aff := range vuln.Affected {
					fixedVersions := []string{}
					for _, affr := range aff.Ranges {
						for _, affre := range affr.Events {
							if affre.Fixed == "" {
								continue
							}
							fixedVersions = append(fixedVersions, fmt.Sprintf("%v", affre.Fixed))
						}
					}
					fixed = append(fixed, fmt.Sprintf("%v", strings.Join(fixedVersions, ", ")))
				}

				// final display
				fmt.Printf(
					"\n%v: %v (fix: %v)\n%v\n",
					pkg,
					text.Color.Sprint(text.FgRed, fmt.Sprintf("(%v) %v", vuln.ID, vuln.Summary)),
					strings.Join(fixed, ", "),
					text.Color.Sprint(text.FgHiBlack, rg.ReplaceAllString(vuln.Details, "\n\n")),
				)
			}
		}

		return nil
	}
	return fmt.Errorf("no such source: %v", args[0])
}

func hash(path string) string {
	h := sha256.New()
	h.Write([]byte(path))
	return fmt.Sprintf("%x", h.Sum(nil))[:12]
}

func newTable() table.Writer {
	t := table.NewWriter()
	t.SetStyle(table.Style{
		Box: table.BoxStyle{
			MiddleHorizontal: "  ",
			MiddleSeparator:  "  ",
			MiddleVertical:   "  ",
		},
		Format: table.FormatOptions{
			Header: text.FormatUpper,
		},
		Options: table.Options{
			SeparateColumns: true,
		},
	})
	return t
}
