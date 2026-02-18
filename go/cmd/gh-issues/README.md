# gh-issues

A command-line tool for creating GitHub issues.

## Features

- Create GitHub issues from the command line
- Support for issue title, body, labels, and assignees
- Uses GitHub personal access token for authentication

## Installation

```bash
go build ./go/cmd/gh-issues
```

## Usage

### Create an Issue

```bash
gh-issues create owner/repo --title "Issue title" --body "Issue description"
```

### Options

- `--title` (required): The title of the issue
- `--body`: The body/description of the issue
- `--labels` or `-l`: Comma-separated list of labels (e.g., `bug,enhancement`)
- `--assignee` or `-a`: Assign the issue to a GitHub user
- `--token` or `-t`: GitHub personal access token (can also use `GITHUB_TOKEN` environment variable)

### Examples

Create a simple issue:
```bash
gh-issues create octocat/Hello-World --title "Fix the bug"
```

Create an issue with body and labels:
```bash
gh-issues create octocat/Hello-World \
  --title "Add new feature" \
  --body "This feature would be great" \
  --labels "enhancement,help wanted"
```

Create an issue and assign it:
```bash
gh-issues create octocat/Hello-World \
  --title "Update documentation" \
  --body "Docs need updating" \
  --assignee "octocat"
```

Using environment variable for token:
```bash
export GITHUB_TOKEN="your_github_token"
gh-issues create owner/repo --title "My issue"
```

## Authentication

You need a GitHub personal access token with `repo` scope to create issues. You can provide it either:

1. Using the `--token` flag
2. Setting the `GITHUB_TOKEN` environment variable

To create a token, go to: https://github.com/settings/tokens
