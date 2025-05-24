package api

// https://endoflife.date/api/v1/products
import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

const api = "https://endoflife.date/api/v1"

type Product struct {
	Name     string   `json:"name"`
	Aliases  []string `json:"alias"`
	Label    string   `json:"label"`
	Category string   `json:"category"`
	Tags     []string `json:"tags"`
	URI      string   `json:"uri"`
}

func (p Product) Title() string {
	return p.Label
}

func (p Product) Description() string {
	return strings.Join(p.Tags, ", ")
}

func (p Product) FilterValue() string {
	return fmt.Sprintf("%v %v %v %v", p.Name, p.Label, strings.Join(p.Tags, " "), strings.Join(p.Aliases, " "))
}

type Products struct {
	Total  int       `json:"total"`
	Result []Product `json:"result"`
}

func ListProducts() (Products, error) {
	resp, err := http.Get(fmt.Sprintf("%s/products", api))
	if err != nil {
		return Products{}, fmt.Errorf("failed to fetch data: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return Products{}, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return Products{}, fmt.Errorf("failed to read response body: %w", err)
	}

	products := Products{}
	if err := json.Unmarshal(body, &products); err != nil {
		return Products{}, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return products, nil
}

type ProductInfo struct {
	Result struct {
		Label  string `json:"label"`
		Labels struct {
			Eoas string `json:"eoas"`
			Eol  string `json:"eol"`
			Eoes string `json:"eoes"`
		} `json:"labels"`
		Releases []struct {
			Name         string `json:"name"`
			ReleaseDate  string `json:"releaseDate"`
			IsLts        bool   `json:"isLts"`
			LtsFrom      string `json:"ltsFrom"`
			IsEoas       bool   `json:"isEoas"`
			EoasFrom     string `json:"eoasFrom"`
			IsEol        bool   `json:"isEol"`
			EolFrom      string `json:"eolFrom"`
			IsEoes       bool   `json:"isEoes"`
			EoesFrom     string `json:"eoesFrom"`
			IsMaintained bool   `json:"isMaintained"`
			Latest       struct {
				Name string `json:"name"`
				Date string `json:"date"`
			} `json:"latest"`
		} `json:"releases"`
	} `json:"result"`
}

func (p ProductInfo) GetLabelsEoads() string {
	return p.Result.Labels.Eoas
}

func GetProductWithURI(uri string) (ProductInfo, error) {
	resp, err := http.Get(uri)
	if err != nil {
		return ProductInfo{}, fmt.Errorf("failed to fetch data: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return ProductInfo{}, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return ProductInfo{}, fmt.Errorf("failed to read response body: %w", err)
	}

	products := ProductInfo{}
	if err := json.Unmarshal(body, &products); err != nil {
		return ProductInfo{}, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return products, nil
}
