package app

import (
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
	gomock "go.uber.org/mock/gomock"

	"github.com/kahlys/codex/template/adminer/internal/api/openapi"
)

func TestServer_Users(t *testing.T) {
	goldUsers := []User{
		{ID: 1, Username: "user1", Password: "pass1"},
		{ID: 2, Username: "user2", Password: "pass2"},
	}

	tests := map[string]struct {
		wantedStore func(db *MockUserStore)
		wantResp    openapi.UsersResponseObject
		wantErr     bool
	}{
		"ok": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().Users(gomock.Any()).Return(goldUsers, nil)
			},
			wantResp: openapi.Users200JSONResponse(toOpenAPIUsers(goldUsers)),
		},
		"store error": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().Users(gomock.Any()).Return(nil, errors.New("users failed"))
			},
			wantResp: openapi.Users500Response{},
			wantErr:  true,
		},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			mockCtrl := gomock.NewController(t)
			defer mockCtrl.Finish()
			mockDBConfig := NewMockUserStore(mockCtrl)

			gw := NewServer(mockDBConfig)
			tt.wantedStore(mockDBConfig)

			resp, err := gw.Users(t.Context(), openapi.UsersRequestObject{})
			assertServerRes(t, tt.wantErr, err, tt.wantResp, resp)
		})
	}
}

func TestServer_User(t *testing.T) {
	tests := map[string]struct {
		wantedStore func(db *MockUserStore)
		wantResp    openapi.UserResponseObject
		wantErr     bool
	}{
		"ok": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().User(gomock.Any(), 1).Return(User{ID: 1, Username: "user1", Password: "pass1"}, nil)
			},
			wantResp: openapi.User200JSONResponse(openapi.User{Id: 1, Username: "user1", Password: "pass1"}),
		},
		"user not found": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().User(gomock.Any(), 1).Return(User{}, ErrNotFound)
			},
			wantResp: openapi.User404Response{},
		},
		"store error": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().User(gomock.Any(), 1).Return(User{}, errors.New("user lookup failed"))
			},
			wantResp: openapi.User500Response{},
			wantErr:  true,
		},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			mockCtrl := gomock.NewController(t)
			defer mockCtrl.Finish()
			mockDBConfig := NewMockUserStore(mockCtrl)

			gw := NewServer(mockDBConfig)
			tt.wantedStore(mockDBConfig)

			resp, err := gw.User(t.Context(), openapi.UserRequestObject{Id: 1})
			assertServerRes(t, tt.wantErr, err, tt.wantResp, resp)
		})
	}
}

func TestServer_CreateUser(t *testing.T) {
	goldUser := User{ID: 1, Username: "user1", Password: "pass1"}

	tests := map[string]struct {
		wantedStore func(db *MockUserStore)
		wantResp    openapi.CreateUserResponseObject
		wantErr     bool
	}{
		"ok": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().CreateUser(gomock.Any(), "user1", "pass1").Return(goldUser, nil)
			},
			wantResp: openapi.CreateUser201JSONResponse(toOpenAPIUser(goldUser)),
		},
		"store error": {
			wantedStore: func(db *MockUserStore) {
				db.EXPECT().CreateUser(gomock.Any(), "user1", "pass1").Return(User{}, errors.New("create user failed"))
			},
			wantResp: openapi.CreateUser500Response{},
			wantErr:  true,
		},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			mockCtrl := gomock.NewController(t)
			defer mockCtrl.Finish()
			mockDBConfig := NewMockUserStore(mockCtrl)

			gw := NewServer(mockDBConfig)
			tt.wantedStore(mockDBConfig)

			resp, err := gw.CreateUser(t.Context(), openapi.CreateUserRequestObject{
				Body: &openapi.CreateUserJSONRequestBody{
					Username: goldUser.Username,
					Password: goldUser.Password,
				},
			})
			assertServerRes(t, tt.wantErr, err, tt.wantResp, resp)
		})
	}
}

func assertServerRes(t *testing.T, wantErr bool, err error, wantRes, resp any) {
	if wantErr {
		assert.Error(t, err)
		assert.Equal(t, wantRes, resp)
		return
	}

	assert.NoError(t, err)
	assert.Equal(t, wantRes, resp)
}
