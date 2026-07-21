package app

import (
	"context"
	"log/slog"
	"net/http"
	"time"

	"github.com/google/uuid"
)

const loggerKey = "logger"

type responseWriter struct {
	http.ResponseWriter
	status int
}

// Capture the status code when it's written
func (rw *responseWriter) WriteHeader(code int) {
	rw.status = code
	rw.ResponseWriter.WriteHeader(code)
}

func (s *Server) mwLog(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()

		logger := s.logger.With(
			"trace_id", uuid.New().String()[:8],
			"method", r.Method,
			"path", r.URL.Path,
		)

		wrapped := &responseWriter{ResponseWriter: w, status: http.StatusOK}

		next.ServeHTTP(
			wrapped,
			r.WithContext(
				context.WithValue(
					r.Context(),
					loggerKey,
					logger,
				),
			),
		)

		logger.Info("HandleRequest", "status", wrapped.status, "duration", time.Since(start))
	})
}

func (s *Server) log(ctx context.Context) *slog.Logger {
	if logger, ok := ctx.Value(loggerKey).(*slog.Logger); ok {
		return logger
	}
	return s.logger
}
