package app

import (
	"context"
	"io"

	"github.com/google/uuid"
	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
	"gopkg.in/yaml.v3"
)

// UploadFile handles file upload, validates that it's a YAML file, parses it, and stores the content in RAM.
func (s *Server) UploadFile(ctx context.Context, req openapi.UploadFileRequestObject) (openapi.UploadFileResponseObject, error) {
	s.log(ctx).Info("UploadingFile")

	if req.Body == nil {
		s.log(ctx).Error("multipart reader is nil")
		return openapi.UploadFile400Response{}, nil
	}

	part, err := req.Body.NextPart()
	if err != nil {
		s.log(ctx).Error("failed to read multipart", "error", err)
		return openapi.UploadFile400Response{}, nil
	}
	defer part.Close()

	filename := part.FileName()
	if filename == "" {
		s.log(ctx).Error("filename is empty")
		return openapi.UploadFile400Response{}, nil
	}

	content, err := io.ReadAll(part)
	if err != nil {
		s.log(ctx).Error("failed to read file content", "error", err)
		return openapi.UploadFile400Response{}, nil
	}

	var data map[string]interface{}
	err = yaml.Unmarshal(content, &data)
	if err != nil {
		s.log(ctx).Error("failed to parse YAML", "error", err)
		return openapi.UploadFile400Response{}, nil
	}

	fileID := uuid.NewString()

	// Store in RAM
	s.fileStoreM.Lock()
	s.fileStore[fileID] = data
	s.fileStoreM.Unlock()

	s.log(ctx).Info("file uploaded successfully", "fileId", fileID, "filename", filename)

	return openapi.UploadFile201JSONResponse{
		FileId:   &fileID,
		Filename: &filename,
	}, nil
}
