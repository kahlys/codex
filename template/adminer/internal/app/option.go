package app

import "log/slog"

// ServerOption defines a function type for configuring the Server with optional parameters.
type ServerOption func(*Server)

// WithLogger allows setting a custom logger for the Server. If no logger is provided, the default logger will be used.
func WithLogger(logger *slog.Logger) ServerOption {
	return func(s *Server) {
		if logger != nil {
			s.logger = logger
		}
	}
}
