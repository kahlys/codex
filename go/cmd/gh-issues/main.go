package main

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/google/go-github/v67/github"
	"github.com/spf13/cobra"
)

var (
	token    string
	title    string
	body     string
	labels   []string
	assignee string
)

func main() {
	root := &cobra.Command{
		Use:   "gh-issues",
		Short: "GitHub Issues CLI tool",
		Long:  "Create and manage GitHub issues from the command line",
		CompletionOptions: cobra.CompletionOptions{
			DisableDefaultCmd: true,
		},
	}

	createCmd := &cobra.Command{
		Use:   "create [owner/repo]",
		Short: "Create a new GitHub issue",
		Long:  "Create a new GitHub issue in the specified repository (format: owner/repo)",
		Args:  cobra.ExactArgs(1),
		RunE:  runCreateCommand,
	}

	createCmd.Flags().StringVarP(&token, "token", "t", "", "GitHub personal access token (or use GITHUB_TOKEN env var)")
	createCmd.Flags().StringVar(&title, "title", "", "Issue title (required)")
	createCmd.Flags().StringVar(&body, "body", "", "Issue body/description")
	createCmd.Flags().StringSliceVarP(&labels, "labels", "l", []string{}, "Comma-separated list of labels")
	createCmd.Flags().StringVarP(&assignee, "assignee", "a", "", "Assign issue to a GitHub user")
	createCmd.MarkFlagRequired("title")

	root.AddCommand(createCmd)

	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}

func runCreateCommand(_ *cobra.Command, args []string) error {
	// Parse owner/repo
	parts := strings.Split(args[0], "/")
	if len(parts) != 2 {
		return fmt.Errorf("invalid repository format, expected 'owner/repo', got '%s'", args[0])
	}
	owner, repo := parts[0], parts[1]

	// Get token from flag or environment
	if token == "" {
		token = os.Getenv("GITHUB_TOKEN")
	}
	if token == "" {
		return fmt.Errorf("GitHub token is required (use --token flag or GITHUB_TOKEN env var)")
	}

	// Create GitHub client
	client := github.NewClient(nil).WithAuthToken(token)
	ctx := context.Background()

	// Prepare issue request
	issueRequest := &github.IssueRequest{
		Title: &title,
	}

	if body != "" {
		issueRequest.Body = &body
	}

	if len(labels) > 0 {
		issueRequest.Labels = &labels
	}

	if assignee != "" {
		issueRequest.Assignee = &assignee
	}

	// Create the issue
	issue, resp, err := client.Issues.Create(ctx, owner, repo, issueRequest)
	if err != nil {
		return fmt.Errorf("failed to create issue: %w", err)
	}

	if resp.StatusCode != 201 {
		return fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	// Print success message
	fmt.Printf("✓ Issue created successfully!\n")
	fmt.Printf("  Number: #%d\n", issue.GetNumber())
	fmt.Printf("  Title:  %s\n", issue.GetTitle())
	fmt.Printf("  URL:    %s\n", issue.GetHTMLURL())

	return nil
}
