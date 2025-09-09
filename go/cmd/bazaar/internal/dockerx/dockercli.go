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
	"github.com/docker/docker/client"
	"github.com/docker/go-connections/nat"
)

const (
	// Label to identify containers created by dboot
	managedByLabelKey   = "managed-by"
	managedByLabelValue = "dboot"
)

var cli *client.Client

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

type Container struct {
	types.Container

	Env map[string]string

	BindingPorts nat.PortMap
	ExitError    string
	ExitCode     int
}

func (c Container) String() string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Container: %s\n", c.ID))
	sb.WriteString(fmt.Sprintf("  Name: %s\n", strings.TrimPrefix(c.Names[0], "/")))
	sb.WriteString(fmt.Sprintf("  Image: %s\n", c.Image))
	sb.WriteString(fmt.Sprintf("  State: %s\n", c.State))
	sb.WriteString(fmt.Sprintf("  Status: %s\n", c.Status))
	sb.WriteString(fmt.Sprintf("  Ports: %v\n", c.Ports))
	sb.WriteString("  Env:\n")
	for k, v := range c.Env {
		sb.WriteString(fmt.Sprintf("    %s=%s\n", k, v))
	}
	sb.WriteString("  BindingPorts:\n")
	for p, bindings := range c.BindingPorts {
		sb.WriteString(fmt.Sprintf("    %v:\n", p))
		for _, b := range bindings {
			sb.WriteString(fmt.Sprintf("      - %s:%s\n", b.HostIP, b.HostPort))
		}
	}
	if c.ExitError != "" {
		sb.WriteString(fmt.Sprintf("  ExitCode: %d\n", c.ExitCode))
		sb.WriteString(fmt.Sprintf("  ExitError: %s\n", c.ExitError))
	}
	return sb.String()
}

func ContainerList(ctx context.Context) ([]Container, error) {
	filter := filters.NewArgs()
	filter.Add("label", fmt.Sprintf("%s=%s", managedByLabelKey, managedByLabelValue))

	containers, err := cli.ContainerList(
		ctx,
		types.ContainerListOptions{
			All:     true,
			Filters: filter,
		},
	)
	if err != nil {
		return nil, err
	}

	result := make([]Container, len(containers))
	for i, c := range containers {
		envvars, _ := containerEnv(ctx, c.ID)

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

func containerEnv(ctx context.Context, containerID string) (map[string]string, error) {
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
	return envVars, nil
}

// ImageExists checks if the given image:tag exists locally
func ImageExists(ctx context.Context, imageRef string) (bool, error) {
	images, err := cli.ImageList(ctx, types.ImageListOptions{})
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

func ContainerStartNew(ctx context.Context, config Config) error {
	imageRef := fmt.Sprintf("%v:%v", config.Image, config.Tag)

	// Use ImageExists helper
	found, err := ImageExists(ctx, imageRef)
	if err != nil {
		return fmt.Errorf("failed to check image existence: %w", err)
	}
	if !found {
		out, err := cli.ImagePull(ctx, imageRef, types.ImagePullOptions{})
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

func ContainerStart(ctx context.Context, containerID string) error {
	if err := cli.ContainerStart(ctx, containerID, types.ContainerStartOptions{}); err != nil {
		return fmt.Errorf("failed to start container: %w", err)
	}
	return nil
}

func ContainerStop(ctx context.Context, containerID string) error {
	if err := cli.ContainerStop(ctx, containerID, container.StopOptions{}); err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	return nil
}

func ContainerStopRemove(ctx context.Context, containerID string) error {
	if err := ContainerStop(ctx, containerID); err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	if err := cli.ContainerRemove(ctx, containerID, types.ContainerRemoveOptions{Force: true}); err != nil {
		return fmt.Errorf("failed to remove container: %w", err)
	}
	return nil
}

func ContainerInspect(ctx context.Context, containerID string) (types.ContainerJSON, error) {
	return cli.ContainerInspect(ctx, containerID)
}

func ContainerNameAvailable(ctx context.Context, name string) error {
	containers, err := cli.ContainerList(
		ctx,
		types.ContainerListOptions{All: true},
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
