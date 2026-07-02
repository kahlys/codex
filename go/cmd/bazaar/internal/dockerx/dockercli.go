// Package dockerx provides utilities for interacting with Docker CLI.
package dockerx

import (
	"context"
	"fmt"
	"io"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/image"
	"github.com/docker/docker/client"
	"github.com/docker/go-connections/nat"
)

const (
	// Label to identify containers created by dboot
	managedByLabelKey   = "managed-by"
	managedByLabelValue = "dboot"
)

var cli *client.Client

// Config describes how a managed container should be created.
type Config struct {
	Name  string
	Image string
	Tag   string

	HostIP        string
	HostPort      string
	ContainerPort string

	Env []string
}

func init() {
	var err error
	cli, err = client.NewClientWithOpts(client.FromEnv)
	if err != nil {
		panic(err)
	}
}

// Container wraps Docker container metadata with inspect-derived details.
type Container struct {
	types.Container

	Env map[string]string

	BindingPorts nat.PortMap
	ExitError    string
	ExitCode     int
}

func (c Container) String() string {
	var sb strings.Builder
	fmt.Fprintf(&sb, "Container: %s\n", c.ID)
	fmt.Fprintf(&sb, "  Name: %s\n", strings.TrimPrefix(c.Names[0], "/"))
	fmt.Fprintf(&sb, "  Image: %s\n", c.Image)
	fmt.Fprintf(&sb, "  State: %s\n", c.State)
	fmt.Fprintf(&sb, "  Status: %s\n", c.Status)
	fmt.Fprintf(&sb, "  Ports: %v\n", c.Ports)
	sb.WriteString("  Env:\n")
	for k, v := range c.Env {
		fmt.Fprintf(&sb, "    %s=%s\n", k, v)
	}
	sb.WriteString("  BindingPorts:\n")
	for p, bindings := range c.BindingPorts {
		fmt.Fprintf(&sb, "    %v:\n", p)
		for _, b := range bindings {
			fmt.Fprintf(&sb, "      - %s:%s\n", b.HostIP, b.HostPort)
		}
	}
	if c.ExitError != "" {
		fmt.Fprintf(&sb, "  ExitCode: %d\n", c.ExitCode)
		fmt.Fprintf(&sb, "  ExitError: %s\n", c.ExitError)
	}
	return sb.String()
}

// ContainerList returns all managed containers with enriched metadata.
func ContainerList(ctx context.Context) ([]Container, error) {
	filter := filters.NewArgs()
	filter.Add("label", fmt.Sprintf("%s=%s", managedByLabelKey, managedByLabelValue))

	containers, err := cli.ContainerList(
		ctx,
		container.ListOptions{
			All:     true,
			Filters: filter,
		},
	)
	if err != nil {
		return nil, err
	}

	result := make([]Container, len(containers))
	for i, c := range containers {
		envvars := containerEnv(ctx, c.ID)

		inspect, _ := cli.ContainerInspect(ctx, c.ID)

		result[i] = Container{
			Container:    c,
			Env:          envvars,
			BindingPorts: inspect.HostConfig.PortBindings,
			ExitError:    inspect.State.Error,
			ExitCode:     inspect.State.ExitCode,
		}
	}
	return result, nil
}

func containerEnv(ctx context.Context, containerID string) map[string]string {
	envVars := make(map[string]string)
	inspect, err := cli.ContainerInspect(ctx, containerID)
	if err == nil {
		for _, env := range inspect.Config.Env {
			if len(env) > 0 {
				parts := strings.SplitN(env, "=", 2)
				if len(parts) == 2 {
					envVars[parts[0]] = parts[1]
				}
			}
		}
	}
	return envVars
}

// ImageExists checks if the given image:tag exists locally
func ImageExists(ctx context.Context, imageRef string) (bool, error) {
	images, err := cli.ImageList(ctx, image.ListOptions{})
	if err != nil {
		return false, err
	}
	for _, img := range images {
		for _, tag := range img.RepoTags {
			if tag == imageRef {
				return true, nil
			}
		}
	}
	return false, nil
}

// ContainerStartNew creates and starts a new managed container.
func ContainerStartNew(ctx context.Context, config Config) error {
	imageRef := fmt.Sprintf("%v:%v", config.Image, config.Tag)

	// Use ImageExists helper
	found, err := ImageExists(ctx, imageRef)
	if err != nil {
		return fmt.Errorf("failed to check image existence: %w", err)
	}
	if !found {
		out, err := cli.ImagePull(ctx, imageRef, image.PullOptions{})
		if err != nil {
			return fmt.Errorf("failed to pull image %s: %w", imageRef, err)
		}
		defer out.Close()
		// Optionally, read the output to completion
		_, _ = io.Copy(io.Discard, out)
	}

	containerPort, err := nat.NewPort("tcp", config.ContainerPort)
	if err != nil {
		return fmt.Errorf("failed to create port: %w", err)
	}

	resp, err := cli.ContainerCreate(
		ctx,
		&container.Config{
			Image: imageRef,
			Env:   config.Env,
			ExposedPorts: nat.PortSet{
				containerPort: struct{}{},
			},
			Labels: map[string]string{
				managedByLabelKey: managedByLabelValue,
			},
		},
		&container.HostConfig{
			PortBindings: nat.PortMap{
				containerPort: []nat.PortBinding{
					{HostIP: config.HostIP, HostPort: config.HostPort},
				},
			},
			RestartPolicy: container.RestartPolicy{
				Name: "unless-stopped",
			},
		},
		nil,
		nil,
		config.Name,
	)
	if err != nil {
		return fmt.Errorf("failed to create container: %w", err)
	}

	if err := ContainerStart(ctx, resp.ID); err != nil {
		return fmt.Errorf("failed to start container: %w", err)
	}

	return nil
}

// ContainerStart starts an existing container.
func ContainerStart(ctx context.Context, containerID string) error {
	if err := cli.ContainerStart(ctx, containerID, container.StartOptions{}); err != nil {
		return fmt.Errorf("failed to start container: %w", err)
	}
	return nil
}

// ContainerStop stops a running container.
func ContainerStop(ctx context.Context, containerID string) error {
	if err := cli.ContainerStop(ctx, containerID, container.StopOptions{}); err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	return nil
}

// ContainerStopRemove stops and removes a container.
func ContainerStopRemove(ctx context.Context, containerID string) error {
	if err := ContainerStop(ctx, containerID); err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	if err := cli.ContainerRemove(ctx, containerID, container.RemoveOptions{Force: true}); err != nil {
		return fmt.Errorf("failed to remove container: %w", err)
	}
	return nil
}

// ContainerInspect returns inspect details for a container.
func ContainerInspect(ctx context.Context, containerID string) (container.InspectResponse, error) {
	return cli.ContainerInspect(ctx, containerID)
}

// ContainerNameAvailable validates that a container name is not already in use.
func ContainerNameAvailable(ctx context.Context, name string) error {
	containers, err := cli.ContainerList(
		ctx,
		container.ListOptions{All: true},
	)
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	for _, c := range containers {
		if len(c.Names) > 0 && strings.TrimPrefix(c.Names[0], "/") == name {
			return fmt.Errorf("container name %s is already in use", name)
		}
	}

	return nil
}
