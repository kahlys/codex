// Package dockerx provides utilities for interacting with Docker CLI.
package dockerx

import (
	"context"
	"fmt"
	"io"
	"net/netip"
	"strings"

	"github.com/moby/moby/api/types/container"
	"github.com/moby/moby/api/types/network"
	"github.com/moby/moby/client"
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
	cli, err = client.New(client.FromEnv)
	if err != nil {
		panic(err)
	}
}

// Container wraps Docker container metadata with inspect-derived details.
type Container struct {
	*container.Summary

	Env          map[string]string
	PortBindings network.PortMap
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
	for p, bindings := range c.PortBindings {
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
	filter := client.Filters{}
	filter.Add("label", fmt.Sprintf("%s=%s", managedByLabelKey, managedByLabelValue))

	containerListResult, err := cli.ContainerList(
		ctx,
		client.ContainerListOptions{
			All:     true,
			Filters: filter,
		},
	)
	if err != nil {
		return nil, err
	}

	result := make([]Container, len(containerListResult.Items))
	for i, c := range containerListResult.Items {
		envvars := containerEnv(ctx, c.ID)

		inspect, _ := cli.ContainerInspect(ctx, c.ID, client.ContainerInspectOptions{})

		result[i] = Container{
			Summary:      &c,
			Env:          envvars,
			PortBindings: inspect.Container.HostConfig.PortBindings,
			ExitError:    inspect.Container.State.Error,
			ExitCode:     inspect.Container.State.ExitCode,
		}
	}
	return result, nil
}

func containerEnv(ctx context.Context, containerID string) map[string]string {
	envVars := make(map[string]string)
	inspect, err := cli.ContainerInspect(ctx, containerID, client.ContainerInspectOptions{})
	if err == nil {
		for _, env := range inspect.Container.Config.Env {
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
	imageListResult, err := cli.ImageList(ctx, client.ImageListOptions{})
	if err != nil {
		return false, err
	}
	for _, img := range imageListResult.Items {
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

	found, err := ImageExists(ctx, imageRef)
	if err != nil {
		return fmt.Errorf("failed to check image existence: %w", err)
	}
	if !found {
		out, err := cli.ImagePull(ctx, imageRef, client.ImagePullOptions{})
		if err != nil {
			return fmt.Errorf("failed to pull image %s: %w", imageRef, err)
		}
		defer out.Close()
		// Optionally, read the output to completion
		_, _ = io.Copy(io.Discard, out)
	}

	containerPort, err := network.ParsePort(config.ContainerPort)
	if err != nil {
		return fmt.Errorf("failed to parse container port: %w", err)
	}

	hostIP, err := netip.ParseAddr(config.HostIP)
	if err != nil {
		return fmt.Errorf("failed to create port: %w", err)
	}

	resp, err := cli.ContainerCreate(
		ctx,
		client.ContainerCreateOptions{
			Name: strings.ToLower(config.Name),
			Config: &container.Config{
				Image: imageRef,
				Env:   config.Env,
				ExposedPorts: network.PortSet{
					containerPort: {},
				},
				Labels: map[string]string{
					managedByLabelKey: managedByLabelValue,
				},
			},
			HostConfig: &container.HostConfig{
				PortBindings: network.PortMap{
					containerPort: {
						{HostIP: hostIP, HostPort: config.HostPort},
					},
				},
				RestartPolicy: container.RestartPolicy{
					Name: "unless-stopped",
				},
			},
		},
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
	_, err := cli.ContainerStart(ctx, containerID, client.ContainerStartOptions{})
	if err != nil {
		return fmt.Errorf("failed to start container: %w", err)
	}
	return nil
}

// ContainerStop stops a running container.
func ContainerStop(ctx context.Context, containerID string) error {
	_, err := cli.ContainerStop(ctx, containerID, client.ContainerStopOptions{})
	if err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	return nil
}

// ContainerStopRemove stops and removes a container.
func ContainerStopRemove(ctx context.Context, containerID string) error {
	if err := ContainerStop(ctx, containerID); err != nil {
		return fmt.Errorf("failed to stop container: %w", err)
	}
	_, err := cli.ContainerRemove(ctx, containerID, client.ContainerRemoveOptions{Force: true})
	if err != nil {
		return fmt.Errorf("failed to remove container: %w", err)
	}
	return nil
}

// ContainerInspect returns inspect details for a container.
func ContainerInspect(ctx context.Context, containerID string) (container.InspectResponse, error) {
	result, err := cli.ContainerInspect(ctx, containerID, client.ContainerInspectOptions{})
	if err != nil {
		return container.InspectResponse{}, err
	}
	return result.Container, nil
}

// ContainerNameAvailable validates that a container name is not already in use.
func ContainerNameAvailable(ctx context.Context, name string) error {
	containerListResult, err := cli.ContainerList(
		ctx,
		client.ContainerListOptions{All: true},
	)
	if err != nil {
		return fmt.Errorf("failed to list containers: %w", err)
	}

	for _, c := range containerListResult.Items {
		if len(c.Names) > 0 && strings.TrimPrefix(c.Names[0], "/") == name {
			return fmt.Errorf("container name %s is already in use", name)
		}
	}

	return nil
}
