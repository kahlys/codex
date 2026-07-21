# builder image
FROM golang AS builder
WORKDIR /myapp
COPY go.mod go.sum ./
COPY template/adminer ./template/adminer
RUN CGO_ENABLED=0 GOOS=linux go build -o myapp ./template/adminer


# final image
FROM alpine
COPY --from=builder /myapp/myapp .
COPY --from=builder /myapp/template/adminer/sql ./sql
ENTRYPOINT [ "./myapp" ]